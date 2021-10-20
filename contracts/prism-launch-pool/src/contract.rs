use crate::state::{
    Config, DistributionStatus, RewardInfo, BOND_AMOUNTS, CONFIG, DISTRIBUTION_STATUS, REWARD_INFO,
};
use crate::vest::{claim_withdrawn_rewards, withdraw_rewards};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::Cw20ReceiveMsg;
use cw20_base::msg::ExecuteMsg as TokenMsg;
use prism_protocol::launch_pool::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
use prism_protocol::yasset_staking::{
    Cw20HookMsg as StakingHookMsg, ExecuteMsg as StakingExecuteMsg, QueryMsg as StakingQueryMsg,
};
use std::cmp::min;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::{query_balance, query_token_balance};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let cfg = Config {
        owner: msg.owner,
        prism_token: msg.prism_token,
        yluna_staking: msg.yluna_staking,
        yluna_token: msg.yluna_token,
        distribution_schedule: msg.distribution_schedule,
    };

    if msg.distribution_schedule.0 > msg.distribution_schedule.1 {
        return Err(StdError::generic_err("invalid distribution schedule"));
    }
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::WithdrawRewards {} => withdraw_rewards(deps, env, info),
        ExecuteMsg::ClaimWithdrawnRewards {} => claim_withdrawn_rewards(deps, env, info),
        ExecuteMsg::AdminWithdrawRewards {} => admin_withdraw_rewards(deps, env, info),
        ExecuteMsg::AdminSendWithdrawnRewards { original_balances } => {
            admin_send_withdrawn_rewards(deps, env, info, &original_balances)
        }
    }
}

pub fn to_asset_balance(
    deps: &DepsMut,
    address: &Addr,
    asset_info: &AssetInfo,
) -> StdResult<Asset> {
    let amount = match asset_info.clone() {
        AssetInfo::Token { contract_addr } => query_token_balance(
            &deps.querier,
            Addr::unchecked(contract_addr),
            address.clone(),
        )?,
        AssetInfo::NativeToken { denom } => query_balance(&deps.querier, address.clone(), denom)?,
    };

    Ok(Asset {
        info: asset_info.clone(),
        amount,
    })
}

pub fn admin_withdraw_rewards(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != cfg.owner.as_str() {
        return Err(StdError::generic_err("unauthorized"));
    }

    let whitelist: Vec<AssetInfo> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: cfg.yluna_staking.clone(),
        msg: to_binary(&StakingQueryMsg::Whitelist {})?,
    }))?;

    let mut balances = vec![];
    for asset_info in whitelist {
        balances.push(to_asset_balance(&deps, &env.contract.address, &asset_info)?);
    }

    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::AdminSendWithdrawnRewards {
                original_balances: balances,
            })?,
            funds: vec![],
        })),
    )
}

pub fn admin_send_withdrawn_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    original_balances: &Vec<Asset>,
) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != env.contract.address.as_str() {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut messages = vec![];
    for prev in original_balances {
        let current = to_asset_balance(&deps, &env.contract.address, &prev.info)?;
        let send_asset = Asset {
            info: prev.info.clone(),
            amount: current.amount - prev.amount,
        };

        if !send_asset.amount.is_zero() {
            messages.push(send_asset.into_msg(&deps.querier, Addr::unchecked(cfg.owner.clone()))?);
        }
    }

    Ok(Response::new().add_messages(messages))
}

pub fn bond(deps: DepsMut, env: Env, sender: &String, amount: Uint128) -> StdResult<Response> {
    update_reward_index(deps.storage, &env)?;
    pull_pending_rewards(deps.storage, &sender)?;
    let cfg = CONFIG.load(deps.storage)?;
    let current_bound = BOND_AMOUNTS.load(deps.storage, sender.as_bytes())?;
    BOND_AMOUNTS.save(deps.storage, sender.as_bytes(), &(current_bound + amount))?;

    Ok(
        Response::new().add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token,
            msg: to_binary(&TokenMsg::Send {
                contract: cfg.yluna_staking,
                amount,
                msg: to_binary(&StakingHookMsg::Bond { mode: None })?,
            })?,
            funds: vec![],
        })]),
    )
}

pub fn unbond(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint128) -> StdResult<Response> {
    update_reward_index(deps.storage, &env)?;
    pull_pending_rewards(deps.storage, &info.sender.clone().into_string())?;
    let cfg = CONFIG.load(deps.storage)?;
    let current_bound = BOND_AMOUNTS.load(deps.storage, info.sender.as_bytes())?;
    BOND_AMOUNTS.save(
        deps.storage,
        info.sender.as_bytes(),
        &(current_bound - amount),
    )?;

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_staking,
            msg: to_binary(&StakingExecuteMsg::Unbond { amount })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token,
            msg: to_binary(&TokenMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
    ]))
}

pub fn _pull_pending_rewards(storage: &dyn Storage, address: &String) -> StdResult<RewardInfo> {
    let distribution_status = DISTRIBUTION_STATUS.load(storage)?;
    let bond_amount = BOND_AMOUNTS
        .load(storage, address.as_bytes())
        .unwrap_or(Uint128::zero());
    let mut reward_info = REWARD_INFO
        .load(storage, address.as_bytes())
        .unwrap_or(RewardInfo {
            index: distribution_status.reward_index,
            pending_reward: Uint128::zero(),
        });
    let pending_reward = (bond_amount * distribution_status.reward_index)
        .checked_sub(bond_amount * reward_info.index)?;
    reward_info.index = distribution_status.reward_index;
    reward_info.pending_reward += pending_reward;
    Ok(reward_info)
}

pub fn pull_pending_rewards(storage: &mut dyn Storage, address: &String) -> StdResult<()> {
    let reward_info = _pull_pending_rewards(storage, &address)?;
    REWARD_INFO.save(storage, address.as_bytes(), &reward_info)
}

pub fn _update_reward_index(storage: &dyn Storage, env: &Env) -> StdResult<DistributionStatus> {
    let cfg = CONFIG.load(storage)?;
    let mut distribution_status = DISTRIBUTION_STATUS.load(storage)?;
    let (start, end, amount) = cfg.distribution_schedule;
    if env.block.time.seconds() < start {
        return Err(StdError::generic_err("has not started distributing yet"));
    };
    let denom = end - start;
    let total_distribute =
        amount.multiply_ratio(min(env.block.time.seconds() - start, denom), denom);
    let mut distribute_here = total_distribute - distribution_status.total_distributed;
    distribution_status.total_distributed = total_distribute;

    if distribution_status.total_bond_amount.is_zero() {
        distribution_status.pending_reward += distribute_here;
    } else {
        distribute_here += distribution_status.pending_reward;
        let normal_reward_per_bond =
            Decimal::from_ratio(distribute_here, distribution_status.total_bond_amount);
        distribution_status.reward_index =
            distribution_status.reward_index + normal_reward_per_bond;
        distribution_status.pending_reward = Uint128::zero();
    }
    Ok(distribution_status)
}

pub fn update_reward_index(storage: &mut dyn Storage, env: &Env) -> StdResult<()> {
    let distribution_status = _update_reward_index(storage, &env)?;
    DISTRIBUTION_STATUS.save(storage, &distribution_status)
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yluna token contract can execute this message
            if cfg.yluna_token != info.sender.to_string() {
                return Err(StdError::generic_err("unauthorized"));
            }

            bond(deps, env, &cw20_msg.sender, cw20_msg.amount)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DistributionStatus {} => to_binary(&_update_reward_index(deps.storage, &env)?),
        QueryMsg::RewardInfo { staker_addr } => {
            to_binary(&_pull_pending_rewards(deps.storage, &staker_addr)?)
        }
    }
}
