use crate::error::ContractError;
use crate::state::{
    Config, DistributionStatus, RewardInfo, BOND_AMOUNTS, CONFIG, DISTRIBUTION_STATUS,
    PENDING_WITHDRAW, REWARD_INFO, SCHEDULED_VEST,
};
use crate::vest::{claim_withdrawn_rewards, withdraw_rewards};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Order, QueryRequest, Response, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::{Asset, AssetInfo};
use prism_protocol::launch_pool::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, VestingStatusResponse,
};
use prism_protocol::yasset_staking::{
    Cw20HookMsg as StakingHookMsg, ExecuteMsg as StakingExecuteMsg, QueryMsg as StakingQueryMsg,
    RewardAssetWhitelistResponse,
};
use std::cmp::min;
use std::convert::TryInto;

const CONTRACT_NAME: &str = "prism-launch-pool";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        prism_token: deps.api.addr_validate(&msg.prism_token)?,
        yluna_staking: deps.api.addr_validate(&msg.yluna_staking)?,
        yluna_token: deps.api.addr_validate(&msg.yluna_token)?,
        distribution_schedule: msg.distribution_schedule,
    };

    if msg.distribution_schedule.0 > msg.distribution_schedule.1 {
        return Err(ContractError::InvalidDistributionSchedule {});
    }
    CONFIG.save(deps.storage, &cfg)?;
    DISTRIBUTION_STATUS.save(deps.storage, &DistributionStatus::default())?;
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg), // Bond
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::WithdrawRewards {} => withdraw_rewards(deps, env, info), // Start vesting period
        ExecuteMsg::ClaimWithdrawnRewards {} => claim_withdrawn_rewards(deps, env, info), // Actually withdraw after rewards have vested
        ExecuteMsg::AdminWithdrawRewards {} => admin_withdraw_rewards(deps, env, info),
        ExecuteMsg::AdminSendWithdrawnRewards { original_balances } => {
            admin_send_withdrawn_rewards(deps, env, info, &original_balances)
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yluna token contract can execute this message
            if cfg.yluna_token != info.sender {
                return Err(ContractError::Unauthorized {});
            }

            bond(deps, env, &cw20_msg.sender, cw20_msg.amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DistributionStatus {} => {
            to_binary(&_update_reward_index(deps.storage, &env)?.as_res())
        }
        QueryMsg::RewardInfo { staker_addr } => {
            to_binary(&_pull_pending_rewards(deps.storage, &staker_addr)?.as_res())
        }
        QueryMsg::VestingStatus { staker_addr } => {
            to_binary(&query_vesting_status(deps, env, staker_addr)?)
        }
    }
}

pub fn to_asset_balance(
    deps: &DepsMut,
    address: &Addr,
    asset_info: &AssetInfo,
) -> StdResult<Asset> {
    let amount = asset_info.query_balance(&deps.querier, address)?;

    Ok(Asset {
        info: asset_info.clone(),
        amount,
    })
}

pub fn admin_withdraw_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != cfg.owner.as_str() {
        return Err(ContractError::Unauthorized {});
    }

    let whitelist_res: RewardAssetWhitelistResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: cfg.yluna_staking.to_string(),
            msg: to_binary(&StakingQueryMsg::RewardAssetWhitelist {})?,
        }))?;

    let mut balances = vec![];
    for asset_info in whitelist_res.assets {
        balances.push(to_asset_balance(&deps, &env.contract.address, &asset_info)?);
    }

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_staking.to_string(),
            msg: to_binary(&StakingExecuteMsg::ClaimRewards {})?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::AdminSendWithdrawnRewards {
                original_balances: balances,
            })?,
            funds: vec![],
        }),
    ]))
}

pub fn admin_send_withdrawn_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    original_balances: &[Asset],
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != env.contract.address.as_str() {
        return Err(ContractError::Unauthorized {});
    }

    let mut messages = vec![];
    for prev in original_balances {
        let current = to_asset_balance(&deps, &env.contract.address, &prev.info)?;
        let send_asset = Asset {
            info: prev.info.clone(),
            amount: current.amount - prev.amount,
        };

        if !send_asset.amount.is_zero() {
            messages.push(send_asset.transfer_msg(cfg.owner.clone())?);
        }
    }

    Ok(Response::new().add_messages(messages))
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    sender: &str,
    amount: Uint128,
) -> Result<Response, ContractError> {
    update_reward_index(deps.storage, &env)?;
    pull_pending_rewards(deps.storage, sender)?;
    let cfg = CONFIG.load(deps.storage)?;
    let current_bond = BOND_AMOUNTS
        .load(deps.storage, sender.as_bytes())
        .unwrap_or_default();
    BOND_AMOUNTS.save(deps.storage, sender.as_bytes(), &(current_bond + amount))?;

    DISTRIBUTION_STATUS.update(deps.storage, |mut item| -> StdResult<DistributionStatus> {
        item.total_bond_amount += amount;

        Ok(item)
    })?;

    Ok(
        Response::new().add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: cfg.yluna_staking.to_string(),
                amount,
                msg: to_binary(&StakingHookMsg::Bond {})?,
            })?,
            funds: vec![],
        })]),
    )
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>, // If None, the user's entire stake will be unbonded.
) -> Result<Response, ContractError> {
    update_reward_index(deps.storage, &env)?;
    pull_pending_rewards(deps.storage, &info.sender.clone().into_string())?;
    let cfg = CONFIG.load(deps.storage)?;
    let current_bond = BOND_AMOUNTS
        .load(deps.storage, info.sender.as_bytes())
        .map_err(|_| ContractError::InvalidUnbond {
            reason: "no tokens bonded".to_string(),
        })?;

    let unbond_amt = match amount {
        Some(amount) => {
            if amount > current_bond {
                return Err(ContractError::InvalidUnbond {
                    reason: "can not unbond more than the bonded amount".to_string(),
                });
            }
            amount
        }
        None => current_bond,
    };

    BOND_AMOUNTS.save(
        deps.storage,
        info.sender.as_bytes(),
        &(current_bond - unbond_amt),
    )?;

    DISTRIBUTION_STATUS.update(deps.storage, |mut item| -> StdResult<DistributionStatus> {
        item.total_bond_amount -= unbond_amt;

        Ok(item)
    })?;

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_staking.to_string(),
            msg: to_binary(&StakingExecuteMsg::Unbond {
                amount: Some(unbond_amt),
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: unbond_amt,
            })?,
            funds: vec![],
        }),
    ]))
}

/// Loads a user's reward_info from storage and returns and updated copy of it.
/// Called on every bond, unbond and withdraw_rewards by this user.
pub fn _pull_pending_rewards(storage: &dyn Storage, address: &str) -> StdResult<RewardInfo> {
    let distribution_status = DISTRIBUTION_STATUS.load(storage)?;
    let bond_amount = BOND_AMOUNTS
        .load(storage, address.as_bytes())
        .unwrap_or_else(|_| Uint128::zero());
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

pub fn pull_pending_rewards(storage: &mut dyn Storage, address: &str) -> StdResult<()> {
    let reward_info = _pull_pending_rewards(storage, address)?;
    REWARD_INFO.save(storage, address.as_bytes(), &reward_info)
}

/// Reads DISTRIBUTION_STATUS from storage and returns an updated copy of it.
/// Called on every bond, unbond and withdraw_rewards (for all users).
pub fn _update_reward_index(storage: &dyn Storage, env: &Env) -> StdResult<DistributionStatus> {
    let cfg = CONFIG.load(storage)?;
    let mut distribution_status = DISTRIBUTION_STATUS.load(storage)?;
    let (start, end, amount_scheduled) = cfg.distribution_schedule;
    if env.block.time.seconds() < start {
        // Nothing to do yet; distribution event will happen in the future.
        return Ok(distribution_status);
    };
    let denom = end - start; // Duration of distribution event in seconds.

    // total_distribute is cumulative reward that should have been released by
    // the protocol since the beginning of the schedule and up to the current
    // time. Units: PRISM tokens. Range: [0, amount_scheduled]
    let total_distribute =
        amount_scheduled.multiply_ratio(min(env.block.time.seconds() - start, denom), denom);
    // distribute_here is amount of rewards that should be released by the
    // protocol since last time update_reward_index was called until now.
    let mut distribute_here = total_distribute - distribution_status.total_distributed;
    distribution_status.total_distributed = total_distribute;

    if distribution_status.total_bond_amount.is_zero() {
        // No bonders. Save these rewards for later to avoid division for zero.
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
    let distribution_status = _update_reward_index(storage, env)?;
    DISTRIBUTION_STATUS.save(storage, &distribution_status)
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg: Config = CONFIG.load(deps.storage)?;

    Ok(cfg.as_res())
}

pub fn query_vesting_status(
    deps: Deps,
    env: Env,
    staker_addr: String,
) -> StdResult<VestingStatusResponse> {
    let current_time = env.block.time.seconds();

    let mut can_withdraw = PENDING_WITHDRAW
        .load(deps.storage, staker_addr.as_bytes())
        .unwrap_or_else(|_| Uint128::zero());
    let mut scheduled_vests: Vec<(u64, Uint128)> = vec![];

    for item in SCHEDULED_VEST.prefix(staker_addr.as_bytes()).range(
        deps.storage,
        None,
        None,
        Order::Ascending,
    ) {
        let (key, unlocked) = item?;
        let end_time = u64::from_be_bytes(key.try_into().unwrap());
        scheduled_vests.push((end_time, unlocked));
        if current_time < end_time {
            break;
        }
        can_withdraw += unlocked;
    }

    Ok(VestingStatusResponse {
        scheduled_vests,
        withdrawable: can_withdraw,
    })
}
