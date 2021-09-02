#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, from_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult,
};

use prism_protocol::yasset_staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};

use crate::rewards::{deposit_rewards, withdraw_reward};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG};

use cw20::Cw20ReceiveMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(
        deps.storage,
        &Config {
            owner: msg.owner,
            yluna_token: msg.yluna_token,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::UpdateConfig { owner } => update_config(deps, info, owner),
        ExecuteMsg::Unbond { amount } => unbond(deps, info.sender.to_string(), amount),
        ExecuteMsg::Withdraw {} => withdraw_reward(deps, info),
        ExecuteMsg::DepositRewards { assets } => deposit_rewards(deps, env, info, assets),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
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

            bond(deps, cw20_msg.sender, cw20_msg.amount)
        }
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.owner = owner.unwrap_or(config.owner);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    // match msg {
    //     QueryMsg::Config {} => to_binary(&query_config(deps)?),
    //     QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
    //     QueryMsg::RewardInfo {
    //         staker_addr,
    //         asset_token,
    //     } => to_binary(&query_reward_info(deps, staker_addr, asset_token)?),
    // }
    Err(StdError::generic_err("cringe"))
}

// pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
//     let state = read_config(deps.storage)?;
//     let resp = ConfigResponse {
//         owner: state.owner,
//         yluna_token: state.yluna_token,
//     };
//
//     Ok(resp)
// }
//
// pub fn query_pool_info(deps: Deps, asset_token: String) -> StdResult<PoolInfoResponse> {
//     let pool_info: PoolInfo = read_pool_info(deps.storage, &asset_token)?;
//     Ok(PoolInfoResponse {
//         asset_token,
//         staking_token: pool_info.staking_token,
//         total_bond_amount: pool_info.total_bond_amount,
//         reward_index: pool_info.reward_index,
//         pending_reward: pool_info.pending_reward,
//     })
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
