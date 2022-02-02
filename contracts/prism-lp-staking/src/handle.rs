#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, DepsMut, Env, MessageInfo, Response, Uint128, WasmMsg,
};
use cosmwasm_std::{Decimal, Order, StdResult, Storage};
use cw20::Cw20ExecuteMsg;
use cw_storage_plus::U64Key;
use prismswap::querier::query_token_balance;

use crate::state::{
    get_unlocked_amount, remove_unlocked, Config, PoolInfo, RewardInfo, CONFIG, LOCK_INFO, POOLS,
    REWARD_INFO, STAKER_BY_TOKEN_INDEXER,
};
use crate::ContractError;

use prism_protocol::de::deserialize_key;

pub fn update_owner(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Addr,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    config.owner = new_owner;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_owner"))
}

pub fn add_distribution_schedule(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    schedule: Vec<(u64, u64, Uint128)>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let current_time = env.block.time.seconds();

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    for schedule in schedule.clone() {
        if schedule.0 < current_time || schedule.1 <= schedule.0 || schedule.2.is_zero() {
            return Err(ContractError::InvalidDistributionSchedule {});
        }
    }

    config.distribution_schedule.extend(schedule);

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "add_distribution_schedule"))
}

pub fn register_staking_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staking_token: Addr,
    lock_period: u64,
    weight: u64,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let current_time = env.block.time.seconds();

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    };

    // update all pools rewards so that the new weight is only applied to all pools from this instant
    let pools: Vec<(Addr, PoolInfo)> = POOLS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (k, pool_info) = item?;
            Ok((deserialize_key::<Addr>(k).unwrap(), pool_info))
        })
        .collect::<StdResult<Vec<(Addr, PoolInfo)>>>()?;

    for item in pools {
        let (pool_staking_token, mut pool) = item;

        if pool_staking_token == staking_token {
            return Err(ContractError::AlreadyExists {});
        }
        compute_pool_reward(&config, &mut pool, current_time);

        POOLS.save(deps.storage, &pool_staking_token, &pool)?;
    }

    // add the new pool
    POOLS.save(
        deps.storage,
        &staking_token,
        &PoolInfo {
            last_distributed: current_time,
            weight,
            lock_period,
            ..PoolInfo::default()
        },
    )?;

    // update total  weight
    config.total_weight += weight;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "register_staking_token"))
}

pub fn update_staking_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staking_token: Addr,
    lock_period: Option<u64>,
    weight: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    };

    if let Some(new_weight) = weight {
        let mut pool_found = false;
        let current_time = env.block.time.seconds();

        // update all pools rewards so that the new weight is only applied to all pools from this instant
        let pools: Vec<(Addr, PoolInfo)> = POOLS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (k, pool_info) = item?;
                Ok((deserialize_key::<Addr>(k).unwrap(), pool_info))
            })
            .collect::<StdResult<Vec<(Addr, PoolInfo)>>>()?;

        for item in pools {
            let (pool_staking_token, mut pool) = item;
            compute_pool_reward(&config, &mut pool, current_time);

            if pool_staking_token == staking_token {
                pool_found = true;

                config.total_weight -= pool.weight;
                config.total_weight += new_weight;
                pool.weight = new_weight;
            }

            POOLS.save(deps.storage, &pool_staking_token, &pool)?;
        }

        if !pool_found {
            return Err(ContractError::InvalidStakingToken {});
        }

        // save config with new total_weight
        CONFIG.save(deps.storage, &config)?;
    }

    if let Some(lock_period) = lock_period {
        let mut pool = POOLS
            .load(deps.storage, &staking_token)
            .map_err(|_| ContractError::InvalidStakingToken {})?;
        pool.lock_period = lock_period;

        POOLS.save(deps.storage, &staking_token, &pool)?;
    }

    Ok(Response::new().add_attribute("action", "update_staking_token"))
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
    pool_info: Option<PoolInfo>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let mut pool_info: PoolInfo = pool_info.unwrap_or(
        POOLS
            .load(deps.storage, &staking_token)
            .map_err(|_| ContractError::InvalidStakingToken {})?,
    );
    let mut staker_reward_info: RewardInfo =
        match REWARD_INFO.may_load(deps.storage, (&sender_addr, &staking_token))? {
            Some(reward_info) => reward_info,
            None => RewardInfo::default(),
        };

    // pulls the expired lock entires and accumulates it into unlocked_amount
    pull_unlocked(
        deps.storage,
        &sender_addr,
        &staking_token,
        &mut staker_reward_info,
        env.block.time.seconds(),
        pool_info.lock_period,
    )?;

    // Compute global pool reward & staker reward
    compute_pool_reward(&config, &mut pool_info, env.block.time.seconds());
    compute_staker_reward(&pool_info, &mut staker_reward_info);

    // Increase bond_amount
    increase_bond_amount(&mut pool_info, &mut staker_reward_info, amount);

    // Store updated state with staker's staker_info
    REWARD_INFO.save(
        deps.storage,
        (&sender_addr, &staking_token),
        &staker_reward_info,
    )?;
    STAKER_BY_TOKEN_INDEXER.save(deps.storage, (&staking_token, &sender_addr), &true)?;

    // store the time the tokens are locked
    LOCK_INFO.save(
        deps.storage,
        (
            &sender_addr,
            &staking_token,
            U64Key::from(env.block.time.seconds()),
        ),
        &amount,
    )?;

    POOLS.save(deps.storage, &staking_token, &pool_info)?;

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

    let mut pool: PoolInfo = POOLS
        .load(deps.storage, &staking_token)
        .map_err(|_| ContractError::InvalidStakingToken {})?;
    let mut staker_reward_info: RewardInfo = REWARD_INFO
        .load(deps.storage, (&info.sender, &staking_token))
        .map_err(|_| ContractError::NothingStaked {})?;

    // pulls the expired lock entires and accumulates it into unlocked_amount
    pull_unlocked(
        deps.storage,
        &info.sender,
        &staking_token,
        &mut staker_reward_info,
        env.block.time.seconds(),
        pool.lock_period,
    )?;

    if staker_reward_info.unlocked_amount.is_zero() {
        return Err(ContractError::NothingAvailableToUnbond {});
    }

    let amount_to_unbond: Uint128 = if let Some(amount) = amount {
        if staker_reward_info.unlocked_amount < amount {
            return Err(ContractError::InvalidUnbondAmount {});
        } else {
            amount
        }
    } else {
        staker_reward_info.unlocked_amount
    };

    // Compute global pool reward & staker reward
    compute_pool_reward(&config, &mut pool, env.block.time.seconds());
    compute_staker_reward(&pool, &mut staker_reward_info);

    // Decrease bond_amount
    decrease_bond_amount(&mut pool, &mut staker_reward_info, amount_to_unbond);

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_reward_info.pending_reward.is_zero() && staker_reward_info.bond_amount.is_zero() {
        REWARD_INFO.remove(deps.storage, (&info.sender, &staking_token));
        STAKER_BY_TOKEN_INDEXER.remove(deps.storage, (&staking_token, &info.sender));
    } else {
        REWARD_INFO.save(
            deps.storage,
            (&info.sender, &staking_token),
            &staker_reward_info,
        )?;
    }

    // Store updated pool
    POOLS.save(deps.storage, &staking_token, &pool)?;

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

    let reward_infos: Vec<(Addr, RewardInfo)> = match staking_token {
        Some(token) => {
            let reward_info = REWARD_INFO
                .load(deps.storage, (&info.sender, &token))
                .map_err(|_| ContractError::InvalidStakingToken {})?;

            vec![(token, reward_info)]
        }
        None => REWARD_INFO
            .prefix(&info.sender)
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (k, reward_info) = item.unwrap();
                let staking_token = deserialize_key::<Addr>(k).unwrap();
                Ok((staking_token, reward_info))
            })
            .collect::<StdResult<Vec<(Addr, RewardInfo)>>>()?,
    };

    let mut claim_amount = Uint128::zero();
    for (staking_token, mut staker_reward_info) in reward_infos {
        let mut pool: PoolInfo = POOLS.load(deps.storage, &staking_token)?;

        // Compute global pool reward & staker reward
        compute_pool_reward(&config, &mut pool, env.block.time.seconds());
        compute_staker_reward(&pool, &mut staker_reward_info);

        claim_amount += staker_reward_info.pending_reward;
        staker_reward_info.pending_reward = Uint128::zero();

        // Store or remove updated rewards info
        // depends on the left pending reward and bond amount
        if staker_reward_info.bond_amount.is_zero() {
            REWARD_INFO.remove(deps.storage, (&info.sender, &staking_token));
            STAKER_BY_TOKEN_INDEXER.remove(deps.storage, (&staking_token, &info.sender));
        } else {
            REWARD_INFO.save(
                deps.storage,
                (&info.sender, &staking_token),
                &staker_reward_info,
            )?;
        }

        // store updated pool
        POOLS.save(deps.storage, &staking_token, &pool)?;
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

pub fn auto_stake_hook(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
) -> Result<Response, ContractError> {
    let staking_token_balance: Uint128 =
        query_token_balance(&deps.querier, &staking_token, &env.contract.address)?;

    let pool_info: PoolInfo = POOLS
        .load(deps.storage, &staking_token)
        .map_err(|_| ContractError::InvalidStakingToken {})?;

    // the amount to stake is the difference between the bond amount and the actual balance of the contract
    let amount = staking_token_balance - pool_info.total_bond_amount;

    bond(
        deps,
        env,
        staking_token,
        sender_addr,
        amount,
        Some(pool_info),
    )
}

fn increase_bond_amount(pool: &mut PoolInfo, staker_info: &mut RewardInfo, amount: Uint128) {
    pool.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

fn decrease_bond_amount(pool: &mut PoolInfo, staker_info: &mut RewardInfo, amount: Uint128) {
    pool.total_bond_amount -= amount;
    staker_info.bond_amount -= amount;
    staker_info.unlocked_amount -= amount;
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

pub fn pull_unlocked(
    storage: &mut dyn Storage,
    staker: &Addr,
    staking_token: &Addr,
    staker_info: &mut RewardInfo,
    current_time: u64,
    lock_period: u64,
) -> StdResult<()> {
    let (unlocked_amount, unlock_list) =
        get_unlocked_amount(storage, staker, staking_token, current_time, lock_period)?;

    staker_info.unlocked_amount += unlocked_amount;
    remove_unlocked(storage, staker, staking_token, unlock_list);

    Ok(())
}
