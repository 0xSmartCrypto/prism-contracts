#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use prism_protocol::yasset_staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolInfoResponse, QueryMsg,
};

use crate::rewards::{deposit_rewards, query_reward_info, withdraw_reward};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG, POOL_INFO, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS};

use crate::swaps::{
    deposit_prism, deposit_reward_denom, process_delegator_rewards, update_reward_denom_balance,
};
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;
use terraswap::asset::AssetInfo;

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
            gov: msg.gov,
            yluna_token: msg.yluna_token,
            prism_token: msg.prism_token.clone(),
            reward_denom: msg.reward_denom,
            prism_pair: msg.prism_pair,
        },
    )?;

    TOTAL_BOND_AMOUNT.save(deps.storage, &Uint128::zero())?;
    WHITELISTED_ASSETS.save(
        deps.storage,
        &vec![AssetInfo::Token {
            contract_addr: msg.prism_token.clone(),
        }],
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
        ExecuteMsg::DepositPrism {} => deposit_prism(deps, env, info),
        ExecuteMsg::UpdateRewardDenomBalance {} => update_reward_denom_balance(deps, env, info),
        ExecuteMsg::ProcessDelegatorRewards {} => process_delegator_rewards(deps, env, info),
        ExecuteMsg::DepositRewardDenom {} => deposit_reward_denom(deps, env, info),
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
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
        QueryMsg::Whitelist {} => to_binary(&query_whitelist(deps)?),
        QueryMsg::RewardInfo { staker_addr } => to_binary(&query_reward_info(deps, staker_addr)?),
    }
}

pub fn query_whitelist(deps: Deps) -> StdResult<Vec<AssetInfo>> {
    WHITELISTED_ASSETS.load(deps.storage)
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn query_pool_info(deps: Deps, asset_token: String) -> StdResult<PoolInfoResponse> {
    let pool_info = POOL_INFO.load(deps.storage, asset_token.as_bytes())?;
    Ok(PoolInfoResponse {
        asset_token,
        reward_index: pool_info.reward_index,
        pending_reward: pool_info.pending_reward,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
