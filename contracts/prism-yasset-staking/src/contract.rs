#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardAssetWhitelistResponse,
};

use crate::rewards::{claim_rewards, deposit_rewards, query_reward_info, whitelist_reward_asset};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG, POOL_INFO, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS};

use crate::swaps::{deposit_minted_pyluna_hook, luna_to_pyluna_hook, process_delegator_rewards};
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;
use terraswap::asset::AssetInfo;

const ALLOWED_STAKING_MODES: &'static [&str] = &["xprism"];

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
            reward_denom: msg.reward_denom,
            protocol_fee: msg.protocol_fee,
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
        ExecuteMsg::Unbond { amount } => unbond(deps, info, amount),
        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, info),
        ExecuteMsg::DepositRewards { assets } => deposit_rewards(deps, env, info, assets),
        ExecuteMsg::ProcessDelegatorRewards {} => process_delegator_rewards(deps, env, info),
        ExecuteMsg::LunaToPylunaHook {} => luna_to_pyluna_hook(deps, env),
        ExecuteMsg::DepositMintedPylunaHook {} => deposit_minted_pyluna_hook(deps, env),
        ExecuteMsg::WhitelistRewardAsset { asset } => whitelist_reward_asset(deps, info, asset),
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
        QueryMsg::RewardAssetWhitelist {} => to_binary(&query_whitelist(deps)?),
        QueryMsg::RewardInfo { staker_addr } => to_binary(&query_reward_info(deps, staker_addr)?),
    }
}

pub fn query_whitelist(deps: Deps) -> StdResult<RewardAssetWhitelistResponse> {
    let whitelist = WHITELISTED_ASSETS.load(deps.storage)?;

    Ok(RewardAssetWhitelistResponse { assets: whitelist })
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        vault: cfg.vault,
        gov: cfg.gov,
        collector: cfg.collector,
        reward_denom: cfg.reward_denom,
        protocol_fee: cfg.protocol_fee,
        cluna_token: cfg.cluna_token,
        yluna_token: cfg.yluna_token,
        pluna_token: cfg.pluna_token,
    })
}

pub fn query_pool_info(deps: Deps, asset_token: String) -> StdResult<PoolInfoResponse> {
    let pool_info = POOL_INFO.load(deps.storage, asset_token.as_bytes())?;

    Ok(PoolInfoResponse {
        asset_token,
        reward_index: pool_info.reward_index,
        pending_reward: pool_info.pending_reward,
    })
}
