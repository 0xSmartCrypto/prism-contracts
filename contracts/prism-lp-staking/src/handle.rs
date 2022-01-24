#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, to_binary, Addr, CanonicalAddr, CosmosMsg, DepsMut, Env, MessageInfo, Response, Uint128,
    WasmMsg,
};
use cosmwasm_std::{Decimal, Order, StdResult};
use cw20::Cw20ExecuteMsg;

use crate::state::{
    Config, PoolInfo, RewardInfo, CONFIG, POOLS, REWARD_INFO, STAKER_BY_TOKEN_INDEXER,
};
use crate::ContractError;

pub fn bond(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(sender_addr.as_str())?;
    let staking_token_raw: CanonicalAddr = deps.api.addr_canonicalize(staking_token.as_str())?;

    let config: Config = CONFIG.load(deps.storage)?;
    let mut pool_info: PoolInfo = POOLS
        .load(deps.storage, staking_token_raw.as_slice())
        .map_err(|_| ContractError::InvalidStakingToken {})?;
    let mut staker_reward_info: RewardInfo = match REWARD_INFO.may_load(
        deps.storage,
        (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
    )? {
        Some(reward_info) => reward_info,
        None => RewardInfo::default(),
    };

    // Compute global pool reward & staker reward
    compute_pool_reward(&config, &mut pool_info, env.block.time.seconds());
    compute_staker_reward(&pool_info, &mut staker_reward_info);

    // Increase bond_amount
    increase_bond_amount(&mut pool_info, &mut staker_reward_info, amount);

    // Store updated state with staker's staker_info
    REWARD_INFO.save(
        deps.storage,
        (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
        &staker_reward_info,
    )?;
    STAKER_BY_TOKEN_INDEXER.save(
        deps.storage,
        (staking_token_raw.as_slice(), sender_addr_raw.as_slice()),
        &true,
    )?;
    POOLS.save(deps.storage, staking_token_raw.as_slice(), &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "bond"),
        attr("staking_token", staking_token),
        attr("staker", sender_addr),
        attr("amount", amount.to_string()),
    ]))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staking_token: Addr,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    let staking_token_raw: CanonicalAddr = deps.api.addr_canonicalize(staking_token.as_str())?;

    let mut pool: PoolInfo = POOLS
        .load(deps.storage, staking_token_raw.as_slice())
        .map_err(|_| ContractError::InvalidStakingToken {})?;
    let mut staker_reward_info: RewardInfo = REWARD_INFO
        .load(
            deps.storage,
            (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
        )
        .map_err(|_| ContractError::NothingStaked {})?;

    let amount_to_unbond: Uint128 = if let Some(amount) = amount {
        if staker_reward_info.bond_amount < amount {
            return Err(ContractError::InvalidUnbondAmount {});
        } else {
            amount
        }
    } else {
        staker_reward_info.bond_amount
    };

    // Compute global pool reward & staker reward
    compute_pool_reward(&config, &mut pool, env.block.time.seconds());
    compute_staker_reward(&pool, &mut staker_reward_info);

    // Decrease bond_amount
    decrease_bond_amount(&mut pool, &mut staker_reward_info, amount_to_unbond);

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_reward_info.pending_reward.is_zero() && staker_reward_info.bond_amount.is_zero() {
        REWARD_INFO.remove(
            deps.storage,
            (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
        );
        STAKER_BY_TOKEN_INDEXER.remove(
            deps.storage,
            (staking_token_raw.as_slice(), sender_addr_raw.as_slice()),
        );
    } else {
        REWARD_INFO.save(
            deps.storage,
            (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
            &staker_reward_info,
        )?;
    }

    // Store updated pool
    POOLS.save(deps.storage, staking_token_raw.as_slice(), &pool)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: staking_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: amount_to_unbond,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            attr("action", "unbond"),
            attr("staking_token", staking_token),
            attr("staker", info.sender),
            attr("amount", amount_to_unbond.to_string()),
        ]))
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staking_token: Option<Addr>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    let reward_infos: Vec<(CanonicalAddr, RewardInfo)> = match staking_token {
        Some(token) => {
            let token_raw = deps.api.addr_canonicalize(token.as_str())?;
            let reward_info = REWARD_INFO
                .load(
                    deps.storage,
                    (sender_addr_raw.as_slice(), token_raw.as_slice()),
                )
                .map_err(|_| ContractError::InvalidStakingToken {})?;

            vec![(token_raw, reward_info)]
        }
        None => REWARD_INFO
            .prefix(sender_addr_raw.as_slice())
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (k, v) = item?;
                let staking_token_raw = CanonicalAddr::from(k);

                Ok((staking_token_raw, v))
            })
            .collect::<StdResult<Vec<(CanonicalAddr, RewardInfo)>>>()?,
    };

    let mut claim_amount = Uint128::zero();
    for (staking_token_raw, mut staker_reward_info) in reward_infos {
        let mut pool: PoolInfo = POOLS.load(deps.storage, staking_token_raw.as_slice())?;

        // Compute global pool reward & staker reward
        compute_pool_reward(&config, &mut pool, env.block.time.seconds());
        compute_staker_reward(&pool, &mut staker_reward_info);

        claim_amount += staker_reward_info.pending_reward;
        staker_reward_info.pending_reward = Uint128::zero();

        // Store or remove updated rewards info
        // depends on the left pending reward and bond amount
        if staker_reward_info.bond_amount.is_zero() {
            REWARD_INFO.remove(
                deps.storage,
                (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
            );
            STAKER_BY_TOKEN_INDEXER.remove(
                deps.storage,
                (staking_token_raw.as_slice(), sender_addr_raw.as_slice()),
            );
        } else {
            REWARD_INFO.save(
                deps.storage,
                (sender_addr_raw.as_slice(), staking_token_raw.as_slice()),
                &staker_reward_info,
            )?;
        }

        // store updated pool
        POOLS.save(deps.storage, staking_token_raw.as_slice(), &pool)?;
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    if !claim_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.prism_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: claim_amount,
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "claim_rewards"),
        attr("staker", info.sender.to_string()),
        attr("claim_amount", claim_amount.to_string()),
    ]))
}

fn increase_bond_amount(pool: &mut PoolInfo, staker_info: &mut RewardInfo, amount: Uint128) {
    pool.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

fn decrease_bond_amount(pool: &mut PoolInfo, staker_info: &mut RewardInfo, amount: Uint128) {
    pool.total_bond_amount -= amount;
    staker_info.bond_amount -= amount;
}

// compute distributed rewards for the pool and update global reward index
pub fn compute_pool_reward(config: &Config, pool: &mut PoolInfo, current_time: u64) {
    let mut distributed_amount: Uint128 = Uint128::zero();
    for s in config.distribution_schedule.iter() {
        if s.0 > current_time || s.1 < pool.last_distributed {
            continue;
        }

        // min(s.1, current_time) - max(s.0, last_distributed)
        let seconds_passed =
            std::cmp::min(s.1, current_time) - std::cmp::max(s.0, pool.last_distributed);

        let num_seconds = s.1 - s.0;

        let pool_distribution_amount = s.2 * Decimal::from_ratio(pool.weight, config.total_weight);
        let distribution_amount_per_second: Decimal =
            Decimal::from_ratio(pool_distribution_amount, num_seconds);
        distributed_amount += distribution_amount_per_second * Uint128::from(seconds_passed);
    }

    pool.last_distributed = current_time;
    if pool.total_bond_amount.is_zero() {
        pool.pending_reward += distributed_amount;
    } else {
        pool.reward_index = pool.reward_index
            + Decimal::from_ratio(
                pool.pending_reward + distributed_amount,
                pool.total_bond_amount,
            );
        pool.pending_reward = Uint128::zero()
    }
}

// withdraw reward to pending reward
pub fn compute_staker_reward(pool: &PoolInfo, staker_info: &mut RewardInfo) {
    let pending_reward: Uint128 = (staker_info.bond_amount * pool.reward_index)
        - (staker_info.bond_amount * staker_info.reward_index);

    staker_info.reward_index = pool.reward_index;
    staker_info.pending_reward += pending_reward;
}
