use crate::error::ContractError;
use crate::querier::query_boost_amount;
use crate::state::{
    Config, DistributionStatus, RewardInfo, BASE_DISTRIBUTION_STATUS, BOND_AMOUNTS,
    BOOST_DISTRIBUTION_STATUS, CONFIG, PENDING_WITHDRAW, REWARD_INFO, SCHEDULED_VEST,
};
use crate::vest::{claim_withdrawn_rewards, withdraw_rewards, withdraw_rewards_bulk};
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Order, QueryRequest, Response, StdResult, Storage, Uint128, WasmMsg,
    WasmQuery,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::{Asset, AssetInfo};
use integer_sqrt::IntegerSquareRoot;
use prism_protocol::launch_pool::{
    ConfigResponse, Cw20HookMsg, DistributionStatusResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    VestingStatusResponse,
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
        operator: deps.api.addr_validate(&msg.operator)?,
        prism_token: deps.api.addr_validate(&msg.prism_token)?,
        yluna_staking: deps.api.addr_validate(&msg.yluna_staking)?,
        yluna_token: deps.api.addr_validate(&msg.yluna_token)?,
        boost_contract: deps.api.addr_validate(&msg.boost_contract)?,
        base_distribution_schedule: msg.base_distribution_schedule,
        boost_distribution_schedule: msg.boost_distribution_schedule,
    };

    if msg.base_distribution_schedule.0 > msg.base_distribution_schedule.1 {
        return Err(ContractError::InvalidDistributionSchedule {});
    }
    if msg.boost_distribution_schedule.0 > msg.boost_distribution_schedule.1 {
        return Err(ContractError::InvalidDistributionSchedule {});
    }

    CONFIG.save(deps.storage, &cfg)?;
    BASE_DISTRIBUTION_STATUS.save(deps.storage, &DistributionStatus::default())?;
    BOOST_DISTRIBUTION_STATUS.save(deps.storage, &DistributionStatus::default())?;

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
        ExecuteMsg::ActivateBoost {} => activate_boost(deps, env, info),
        ExecuteMsg::WithdrawRewards {} => withdraw_rewards(deps, env, info),
        ExecuteMsg::ClaimWithdrawnRewards {} => claim_withdrawn_rewards(deps, env, info),
        ExecuteMsg::AdminWithdrawRewards {} => admin_withdraw_rewards(deps, env, info),
        ExecuteMsg::AdminSendWithdrawnRewards { original_balances } => {
            admin_send_withdrawn_rewards(deps, env, info, &original_balances)
        }
        ExecuteMsg::WithdrawRewardsBulk {
            limit,
            start_after_address,
        } => withdraw_rewards_bulk(deps, env, info, limit, start_after_address),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg = cw20_msg.msg;
    let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yluna token contract can execute this message
            if cfg.yluna_token != info.sender {
                return Err(ContractError::Unauthorized {});
            }

            bond(deps, env, cw20_sender, cw20_msg.amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DistributionStatus {} => to_binary(&query_distribution_status(deps, env)?),
        QueryMsg::RewardInfo { staker_addr } => {
            let staker_addr = deps.api.addr_validate(&staker_addr)?;
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
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_reward_indexes(deps.storage, &env, &cfg)?;

    // accumulate accrued rewards
    let mut reward_info = _pull_pending_rewards(deps.storage, &sender)?;

    // update yluna bond amount
    let current_bond = BOND_AMOUNTS
        .load(deps.storage, sender.as_bytes())
        .unwrap_or_default();
    let new_bond_amount = current_bond + amount;
    BOND_AMOUNTS.save(deps.storage, sender.as_bytes(), &new_bond_amount)?;

    BASE_DISTRIBUTION_STATUS.update(deps.storage, |mut item| -> StdResult<DistributionStatus> {
        item.total_weight += amount;

        Ok(item)
    })?;

    // update boost weight
    let boost_amount =
        query_boost_amount(&deps.querier, &cfg.boost_contract, &sender).unwrap_or(Uint128::zero());
    let new_boost_weight =
        Uint128::from((new_bond_amount.u128() * boost_amount.u128()).integer_sqrt());

    BOOST_DISTRIBUTION_STATUS.update(
        deps.storage,
        |mut item| -> StdResult<DistributionStatus> {
            item.total_weight = item.total_weight - reward_info.boost_weight + new_boost_weight;

            Ok(item)
        },
    )?;

    reward_info.boost_weight = new_boost_weight;
    reward_info.active_boost = boost_amount;
    REWARD_INFO.save(deps.storage, sender.as_bytes(), &reward_info)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: cfg.yluna_staking.to_string(),
                amount,
                msg: to_binary(&StakingHookMsg::Bond {})?,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            attr("action", "yluna_farming_bond"),
            attr("total_user_bonded", new_bond_amount.to_string()),
            attr("boost_amount", boost_amount.to_string()),
        ]))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_reward_indexes(deps.storage, &env, &cfg)?;

    // accumulate accrued rewards
    let mut reward_info = _pull_pending_rewards(deps.storage, &info.sender)?;

    // update yluna bond amount
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

    let new_bond_amount = current_bond - unbond_amt;
    BOND_AMOUNTS.save(deps.storage, info.sender.as_bytes(), &new_bond_amount)?;

    BASE_DISTRIBUTION_STATUS.update(deps.storage, |mut item| -> StdResult<DistributionStatus> {
        item.total_weight -= unbond_amt;

        Ok(item)
    })?;

    // update boost weight
    let boost_amount = query_boost_amount(&deps.querier, &cfg.boost_contract, &info.sender)
        .unwrap_or(Uint128::zero());
    let new_boost_weight =
        Uint128::from((new_bond_amount.u128() * boost_amount.u128()).integer_sqrt());

    BOOST_DISTRIBUTION_STATUS.update(
        deps.storage,
        |mut item| -> StdResult<DistributionStatus> {
            item.total_weight = item.total_weight - reward_info.boost_weight + new_boost_weight;

            Ok(item)
        },
    )?;

    reward_info.boost_weight = new_boost_weight;
    reward_info.active_boost = boost_amount;
    REWARD_INFO.save(deps.storage, info.sender.as_bytes(), &reward_info)?;

    Ok(Response::new()
        .add_messages(vec![
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
        ])
        .add_attributes(vec![
            attr("action", "yluna_farming_unbond"),
            attr("total_user_bonded", new_bond_amount.to_string()),
            attr("boost_amount", boost_amount.to_string()),
        ]))
}

pub fn activate_boost(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_reward_indexes(deps.storage, &env, &cfg)?;

    // accumulate accrued rewards
    let mut reward_info = _pull_pending_rewards(deps.storage, &info.sender)?;

    // update yluna bond amount
    let current_bond = BOND_AMOUNTS
        .load(deps.storage, info.sender.as_bytes())
        .unwrap_or_default();

    // update boost weight
    let boost_amount = query_boost_amount(&deps.querier, &cfg.boost_contract, &info.sender)
        .unwrap_or(Uint128::zero());
    let new_boost_weight =
        Uint128::from((current_bond.u128() * boost_amount.u128()).integer_sqrt());

    BOOST_DISTRIBUTION_STATUS.update(
        deps.storage,
        |mut item| -> StdResult<DistributionStatus> {
            item.total_weight = item.total_weight - reward_info.boost_weight + new_boost_weight;

            Ok(item)
        },
    )?;

    reward_info.boost_weight = new_boost_weight;
    reward_info.active_boost = boost_amount;
    REWARD_INFO.save(deps.storage, info.sender.as_bytes(), &reward_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "activate_boost"),
        attr("boost_amount", boost_amount.to_string()),
    ]))
}

pub fn _pull_pending_rewards(storage: &dyn Storage, address: &Addr) -> StdResult<RewardInfo> {
    let base_distribution_status = BASE_DISTRIBUTION_STATUS.load(storage)?;
    let boost_distribution_status = BOOST_DISTRIBUTION_STATUS.load(storage)?;

    let bond_amount = BOND_AMOUNTS
        .load(storage, address.as_bytes())
        .unwrap_or_else(|_| Uint128::zero());
    let mut reward_info = REWARD_INFO
        .load(storage, address.as_bytes())
        .unwrap_or(RewardInfo {
            base_index: base_distribution_status.reward_index,
            boost_index: boost_distribution_status.reward_index,
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
            pending_reward: Uint128::zero(),
        });

    let base_pending_reward = (bond_amount * base_distribution_status.reward_index)
        .checked_sub(bond_amount * reward_info.base_index)?;
    let boost_pending_reward = (reward_info.boost_weight * boost_distribution_status.reward_index)
        .checked_sub(reward_info.boost_weight * reward_info.boost_index)?;

    // accumulate pending reward
    reward_info.pending_reward += base_pending_reward + boost_pending_reward;

    // set user indexes
    reward_info.base_index = base_distribution_status.reward_index;
    reward_info.boost_index = boost_distribution_status.reward_index;

    Ok(reward_info)
}

pub fn _update_reward_index(
    env: &Env,
    distribution_status: &mut DistributionStatus,
    distribution_schedule: (u64, u64, Uint128),
) -> StdResult<()> {
    let (start, end, amount) = distribution_schedule;
    if env.block.time.seconds() < start {
        return Ok(());
    };
    let denom = end - start;
    let total_distribute =
        amount.multiply_ratio(min(env.block.time.seconds() - start, denom), denom);
    let mut distribute_here = total_distribute - distribution_status.total_distributed;
    distribution_status.total_distributed = total_distribute;

    if distribution_status.total_weight.is_zero() {
        distribution_status.pending_reward += distribute_here;
    } else {
        distribute_here += distribution_status.pending_reward;
        let normal_reward_per_bond =
            Decimal::from_ratio(distribute_here, distribution_status.total_weight);
        distribution_status.reward_index =
            distribution_status.reward_index + normal_reward_per_bond;
        distribution_status.pending_reward = Uint128::zero();
    }
    Ok(())
}

pub fn update_reward_indexes(storage: &mut dyn Storage, env: &Env, cfg: &Config) -> StdResult<()> {
    let mut base_distribution_status = BASE_DISTRIBUTION_STATUS.load(storage)?;
    let mut boost_distribution_status = BOOST_DISTRIBUTION_STATUS.load(storage)?;

    // update base global index
    _update_reward_index(
        env,
        &mut base_distribution_status,
        cfg.base_distribution_schedule,
    )?;
    BASE_DISTRIBUTION_STATUS.save(storage, &base_distribution_status)?;
    // update boost global index
    _update_reward_index(
        env,
        &mut boost_distribution_status,
        cfg.boost_distribution_schedule,
    )?;
    BOOST_DISTRIBUTION_STATUS.save(storage, &boost_distribution_status)?;

    Ok(())
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg: Config = CONFIG.load(deps.storage)?;

    Ok(cfg.as_res())
}

pub fn query_distribution_status(deps: Deps, env: Env) -> StdResult<DistributionStatusResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut base_distribution_status = BASE_DISTRIBUTION_STATUS.load(deps.storage)?;
    let mut boost_distribution_status = BOOST_DISTRIBUTION_STATUS.load(deps.storage)?;

    // update base global index
    _update_reward_index(
        &env,
        &mut base_distribution_status,
        cfg.base_distribution_schedule,
    )?;
    // update boost global index
    _update_reward_index(
        &env,
        &mut boost_distribution_status,
        cfg.boost_distribution_schedule,
    )?;

    Ok(DistributionStatusResponse {
        base: base_distribution_status.as_res(),
        boost: boost_distribution_status.as_res(),
    })
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
