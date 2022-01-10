#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128,
};

use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardAssetWhitelistResponse,
};

use crate::rewards::{
    claim_rewards, deposit_rewards, query_reward_info, remove_whitelisted_reward_asset,
    whitelist_reward_asset,
};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG, POOL_INFO, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS};

use crate::swaps::{deposit_minted_pyluna_hook, luna_to_pyluna_hook, process_delegator_rewards};
use astroport::asset::AssetInfo;
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;

const CONTRACT_NAME: &str = "prism-yasset-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONFIG.save(
        deps.storage,
        &Config {
            vault: deps.api.addr_validate(&msg.vault)?,
            gov: deps.api.addr_validate(&msg.gov)?,
            collector: deps.api.addr_validate(&msg.collector)?,
            reward_denom: msg.reward_denom,
            protocol_fee: msg.protocol_fee,
            cluna_token: deps.api.addr_validate(&msg.cluna_token)?,
            yluna_token: deps.api.addr_validate(&msg.yluna_token)?,
            pluna_token: deps.api.addr_validate(&msg.pluna_token)?,
            prism_token: deps.api.addr_validate(&msg.prism_token)?,
            withdraw_fee: validate_rate(msg.withdraw_fee)?,
        },
    )?;

    TOTAL_BOND_AMOUNT.save(deps.storage, &Uint128::zero())?;
    WHITELISTED_ASSETS.save(
        deps.storage,
        &vec![
            AssetInfo::Token {
                contract_addr: deps.api.addr_validate(&msg.pluna_token)?,
            },
            AssetInfo::Token {
                contract_addr: deps.api.addr_validate(&msg.yluna_token)?,
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
        ExecuteMsg::RemoveRewardAsset { asset } => {
            remove_whitelisted_reward_asset(deps, info, asset)
        }
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
        vault: cfg.vault.to_string(),
        gov: cfg.gov.to_string(),
        collector: cfg.collector.to_string(),
        reward_denom: cfg.reward_denom,
        protocol_fee: cfg.protocol_fee,
        cluna_token: cfg.cluna_token.to_string(),
        yluna_token: cfg.yluna_token.to_string(),
        pluna_token: cfg.pluna_token.to_string(),
        prism_token: cfg.prism_token.to_string(),
        withdraw_fee: cfg.withdraw_fee,
    })
}

pub fn query_pool_info(deps: Deps, asset_token: String) -> StdResult<PoolInfoResponse> {
    let pool_info = POOL_INFO.load(deps.storage, asset_token.as_bytes())?;

    Ok(PoolInfoResponse {
        asset_token,
        reward_index: pool_info.reward_index,
    })
}

fn validate_rate(rate: Decimal) -> StdResult<Decimal> {
    if rate > Decimal::one() {
        return Err(StdError::generic_err(format!(
            "Rate can not be bigger than one (given value: {})",
            rate
        )));
    }

    Ok(rate)
}
