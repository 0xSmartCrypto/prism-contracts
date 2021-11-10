#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CanonicalAddr, Deps, DepsMut, Env, MessageInfo, Response,
};
use cw20::Cw20ReceiveMsg;

use crate::error::ContractError;
use crate::handle::{bond, claim_rewards, unbond};
use crate::query::{query_config, query_pool_info, query_staker_info, query_token_stakers_info};
use crate::state::{Config, PoolInfo, CONFIG, LAST_DISTRIBUTED, POOLS};

use prism_protocol::lp_staking::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        prism_token: deps.api.addr_validate(&msg.prism_token)?,
        distribution_schedule: msg.distribution_schedule,
        staking_tokens: msg
            .staking_tokens
            .iter()
            .map(|item| (deps.api.addr_validate(&item.0).unwrap(), item.1))
            .collect(),
        total_weight: msg.staking_tokens.iter().map(|item| item.1).sum(),
    };

    for (staking_token, weight) in &config.staking_tokens {
        let staking_token_raw: CanonicalAddr =
            deps.api.addr_canonicalize(staking_token.as_str())?;
        POOLS.save(
            deps.storage,
            staking_token_raw.as_slice(),
            &PoolInfo {
                last_distributed: env.block.time.seconds(),
                weight: *weight,
                ..PoolInfo::default()
            },
        )?;
    }
    CONFIG.save(deps.storage, &config)?;
    LAST_DISTRIBUTED.save(deps.storage, &env.block.time.seconds())?;

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
        ExecuteMsg::Unbond {
            staking_token,
            amount,
        } => {
            let staking_token_addr = deps.api.addr_validate(&staking_token)?;

            unbond(deps, env, info, staking_token_addr, amount)
        }
        ExecuteMsg::ClaimRewards { staking_token } => {
            let staking_token_addr: Option<Addr> =
                staking_token.map(|item| deps.api.addr_validate(&item).unwrap());

            claim_rewards(deps, env, info, staking_token_addr)
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
        Ok(Cw20HookMsg::Bond {}) => bond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
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
    }
}
