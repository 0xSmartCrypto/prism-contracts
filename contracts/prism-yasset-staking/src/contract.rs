#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

use prism_protocol::yasset_staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};

use crate::rewards::{deposit_rewards, withdraw_reward};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG};

use crate::swaps::{deposit_prism, swap_to_prism, swap_to_reward_denom};
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;

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
            vault: msg.vault,
            yluna_token: msg.yluna_token,
            prism_token: msg.prism_token,
            reward_denom: msg.reward_denom,
            prism_pair: msg.prism_pair,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response<TerraMsgWrapper>> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, info.sender.to_string(), amount),
        ExecuteMsg::Withdraw {} => withdraw_reward(deps, info),
        ExecuteMsg::DepositRewards { assets } => deposit_rewards(deps, env, info, assets),
        ExecuteMsg::SwapToRewardDenom {} => swap_to_reward_denom(deps, env, info),
        ExecuteMsg::SwapToPrism {} => swap_to_prism(deps, env, info),
        ExecuteMsg::DepositPrism { old_amount } => deposit_prism(deps, env, info, old_amount),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response<TerraMsgWrapper>> {
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
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
