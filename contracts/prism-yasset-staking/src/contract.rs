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

use crate::swaps::{convert_and_deposit_cluna, luna_to_cluna, process_delegator_rewards};
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;
use terraswap::asset::AssetInfo;

const ALLOWED_STAKING_MODES: &'static [&str] = &[ "xprism" ];

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
            collector: msg.collector,
            // TODO -- query vault for these addresses
            cluna_token: msg.cluna_token,
            yluna_token: msg.yluna_token.clone(),
            pluna_token: msg.pluna_token.clone(),
        },
    )?;

    TOTAL_BOND_AMOUNT.save(deps.storage, &Uint128::zero())?;
    WHITELISTED_ASSETS.save(
        deps.storage,
        &vec![
            AssetInfo::Token {
                contract_addr: msg.pluna_token.clone(),
            },
            AssetInfo::Token {
                contract_addr: msg.yluna_token.clone(),
            },
        ],
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
        ExecuteMsg::ProcessDelegatorRewards {} => process_delegator_rewards(deps, env, info),
        ExecuteMsg::LunaToCluna {} => luna_to_cluna(deps, env),
        ExecuteMsg::ConvertAndDepositCluna {} => convert_and_deposit_cluna(deps, env),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response<TerraMsgWrapper>> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond { mode } => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yluna token contract can execute this message
            if cfg.yluna_token != info.sender.to_string() {
                return Err(StdError::generic_err("unauthorized"));
            }

            let m = mode.clone();
            if m.is_some() && !ALLOWED_STAKING_MODES.contains(&m.unwrap().as_str()) {
                return Err(StdError::generic_err("unregistered staking mode"));
            }

            bond(deps, cw20_msg.sender, cw20_msg.amount, mode)
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
