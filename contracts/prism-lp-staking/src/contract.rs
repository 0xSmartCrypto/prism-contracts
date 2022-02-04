#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

use crate::error::ContractError;
use crate::handle::{
    add_distribution_schedule, auto_stake_hook, bond, claim_rewards, claim_unbonded,
    register_staking_token, unbond, update_owner, update_staking_token,
};
use crate::query::{
    query_config, query_pool_info, query_staker_info, query_token_stakers_info, query_unbond_orders,
};
use crate::state::{Config, PoolInfo, CONFIG, POOLS};

use prism_protocol::lp_staking::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

const CONTRACT_NAME: &str = "prism-lp-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // validate distribution schedule
    for schedule in msg.distribution_schedule.clone() {
        if schedule.1 <= schedule.0 {
            return Err(ContractError::InvalidDistributionSchedule {});
        }
    }

    let mut staking_tokens = msg.staking_tokens.clone();
    staking_tokens.sort();
    staking_tokens.dedup_by(|item1, item2| item1.0 == item2.0);
    if staking_tokens.len() != msg.staking_tokens.len() {
        return Err(ContractError::DuplicateStakingToken {});
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        prism_token: deps.api.addr_validate(&msg.prism_token)?,
        distribution_schedule: msg.distribution_schedule,
        total_weight: staking_tokens.iter().map(|item| item.1).sum(),
    };

    for (staking_token, weight, unbond_period) in staking_tokens {
        POOLS.save(
            deps.storage,
            &deps.api.addr_validate(&staking_token)?,
            &PoolInfo {
                last_distributed: env.block.time.seconds(),
                weight,
                unbond_period,
                ..PoolInfo::default()
            },
        )?;
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateOwner { owner } => {
            let new_owner_addr = deps.api.addr_validate(&owner)?;

            update_owner(deps, info, new_owner_addr)
        }
        ExecuteMsg::AddDistributionSchedule { schedule } => {
            add_distribution_schedule(deps, env, info, schedule)
        }
        ExecuteMsg::RegisterStakingToken {
            staking_token,
            unbond_period,
            weight,
        } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;

            register_staking_token(deps, env, info, staking_token_addr, unbond_period, weight)
        }
        ExecuteMsg::UpdateStakingToken {
            staking_token,
            unbond_period,
            weight,
        } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;

            update_staking_token(deps, env, info, staking_token_addr, unbond_period, weight)
        }
        ExecuteMsg::Unbond {
            staking_token,
            amount,
        } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;

            unbond(deps, env, info, staking_token_addr, amount)
        }
        ExecuteMsg::ClaimUnbonded { staking_token } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;

            claim_unbonded(deps, env, info, staking_token_addr)
        }
        ExecuteMsg::ClaimRewards { staking_token } => {
            let staking_token_addr: Option<Addr> =
                staking_token.map(|item| deps.api.addr_validate(&item).unwrap());

            claim_rewards(deps, env, info, staking_token_addr)
        }
        ExecuteMsg::AutoStakeHook { staking_token } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;

            auto_stake_hook(deps, env, staking_token_addr, info.sender)
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let cw20_sender: Addr = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => {
            bond(deps, env, info.sender, cw20_sender, cw20_msg.amount, None)
        }
        Err(_) => Err(ContractError::InvalidCw20Msg {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::PoolInfo { staking_token } => {
            let staking_token_addr: Addr = deps.api.addr_validate(&staking_token)?;

            Ok(to_binary(&query_pool_info(deps, staking_token_addr)?)?)
        }
        QueryMsg::StakerInfo {
            staker,
            staking_token,
        } => {
            let staker_addr: Addr = deps.api.addr_validate(&staker)?;
            let staking_token_addr: Option<Addr> =
                staking_token.map(|item| deps.api.addr_validate(&item).unwrap());

            Ok(to_binary(&query_staker_info(
                deps,
                env,
                staker_addr,
                staking_token_addr,
            )?)?)
        }
        QueryMsg::TokenStakersInfo {
            staking_token,
            start_after,
            limit,
        } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;
            let start_after_addr: Option<Addr> =
                start_after.map(|item| deps.api.addr_validate(&item).unwrap());

            Ok(to_binary(&query_token_stakers_info(
                deps,
                env,
                staking_token_addr,
                start_after_addr,
                limit,
            )?)?)
        }
        QueryMsg::UnbondOrders {
            staking_token,
            staker,
            start_after,
            limit,
        } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;
            let staker_addr = deps.api.addr_validate(&staker)?;

            Ok(to_binary(&query_unbond_orders(
                deps,
                env,
                staking_token_addr,
                staker_addr,
                start_after,
                limit,
            )?)?)
        }
    }
}
