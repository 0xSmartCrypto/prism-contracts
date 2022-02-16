use std::str::FromStr;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Decimal, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};

use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardAssetWhitelistResponse, MAX_PROTOCOL_FEE,
};

use crate::rewards::{
    claim_rewards, convert_and_claim_rewards, deposit_rewards, mint_xprism_claim_hook,
    query_reward_info, remove_whitelisted_reward_asset, whitelist_reward_asset,
};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG, POOL_INFO, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS};

use crate::swaps::{deposit_minted_pyluna_hook, luna_to_pyluna_hook, process_delegator_rewards};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw_asset::AssetInfo;
use prismswap::asset::PrismSwapAssetInfo;
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

    validate_protocol_fee(msg.protocol_fee)?;

    CONFIG.save(
        deps.storage,
        &Config {
            owner: deps.api.addr_validate(&msg.owner)?,
            vault: deps.api.addr_validate(&msg.vault)?,
            gov: deps.api.addr_validate(&msg.gov)?,
            collector: deps.api.addr_validate(&msg.collector)?,
            protocol_fee: msg.protocol_fee,
            cluna_token: deps.api.addr_validate(&msg.cluna_token)?,
            yluna_token: deps.api.addr_validate(&msg.yluna_token)?,
            pluna_token: deps.api.addr_validate(&msg.pluna_token)?,
            prism_token: deps.api.addr_validate(&msg.prism_token)?,
            xprism_token: deps.api.addr_validate(&msg.xprism_token)?,
        },
    )?;

    TOTAL_BOND_AMOUNT.save(deps.storage, &Uint128::zero())?;
    WHITELISTED_ASSETS.save(
        deps.storage,
        &vec![
            AssetInfo::Cw20(deps.api.addr_validate(&msg.pluna_token)?),
            AssetInfo::Cw20(deps.api.addr_validate(&msg.yluna_token)?),
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
        // Public endpoints (wide open to the entire internet).
        ExecuteMsg::Unbond { amount } => unbond(deps, info, amount),
        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, info),
        ExecuteMsg::DepositRewards { assets } => {
            for asset in &assets {
                asset.info.check(deps.api)?;
            }
            deposit_rewards(deps, env, info, assets)
        }
        ExecuteMsg::ConvertAndClaimRewards { claim_asset } => {
            claim_asset.check(deps.api)?;
            convert_and_claim_rewards(deps, env, info, claim_asset)
        }
        ExecuteMsg::ProcessDelegatorRewards {} => process_delegator_rewards(deps, env, info),
        ExecuteMsg::LunaToPylunaHook {} => luna_to_pyluna_hook(deps, env),
        _ => {
            // Private endpoints (open to specific callers only).
            match msg {
                ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg), // Bond
                ExecuteMsg::MintXprismClaimHook {
                    receiver,
                    prev_balance,
                } => mint_xprism_claim_hook(deps, info, env, receiver, prev_balance),
                ExecuteMsg::WhitelistRewardAsset { asset } => {
                    asset.check(deps.api)?;
                    whitelist_reward_asset(deps, info, asset)
                }
                ExecuteMsg::RemoveRewardAsset { asset } => {
                    asset.check(deps.api)?;
                    remove_whitelisted_reward_asset(deps, info, asset)
                }
                ExecuteMsg::DepositMintedPylunaHook {
                    prev_pluna_balance,
                    prev_yluna_balance,
                } => deposit_minted_pyluna_hook(deps, info, env, prev_pluna_balance, prev_yluna_balance),
                ExecuteMsg::UpdateConfig {
                    owner,
                    collector,
                    protocol_fee,
                } => update_config(deps, info, owner, collector, protocol_fee),
                _ => Err(StdError::generic_err("not implemented")),
            }
        },
    }
}

/// Accept yluna from the user and bond it in this contract. The user starts
/// accruing rewards in return.
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
            if cfg.yluna_token != info.sender {
                return Err(StdError::generic_err("unauthorized"));
            }

            bond(deps, cw20_msg.sender, cw20_msg.amount)
        }
    }
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    collector: Option<String>,
    protocol_fee: Option<Decimal>,
) -> StdResult<Response<TerraMsgWrapper>> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by owner
    if info.sender != cfg.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        cfg.owner = deps.api.addr_validate(&owner)?;
    }

    if let Some(collector) = collector {
        cfg.collector = deps.api.addr_validate(&collector)?;
    }

    if let Some(protocol_fee) = protocol_fee {
        validate_protocol_fee(protocol_fee)?;
        cfg.protocol_fee = protocol_fee;
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
        QueryMsg::RewardAssetWhitelist {} => to_binary(&query_whitelist(deps)?),
        QueryMsg::RewardInfo { staker_addr } => to_binary(&query_reward_info(deps, staker_addr)?),
        QueryMsg::BondAmount {} => to_binary(&query_bond_amount(deps)?),
    }
}

pub fn query_whitelist(deps: Deps) -> StdResult<RewardAssetWhitelistResponse> {
    let whitelist = WHITELISTED_ASSETS.load(deps.storage)?;

    Ok(RewardAssetWhitelistResponse { assets: whitelist })
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        vault: cfg.vault.to_string(),
        gov: cfg.gov.to_string(),
        collector: cfg.collector.to_string(),
        protocol_fee: cfg.protocol_fee,
        cluna_token: cfg.cluna_token.to_string(),
        yluna_token: cfg.yluna_token.to_string(),
        pluna_token: cfg.pluna_token.to_string(),
        prism_token: cfg.prism_token.to_string(),
        xprism_token: cfg.xprism_token.to_string(),
    })
}

pub fn query_pool_info(deps: Deps, asset_token: String) -> StdResult<PoolInfoResponse> {
    let pool_info = POOL_INFO.load(deps.storage, asset_token.as_bytes())?;

    Ok(PoolInfoResponse {
        asset_token,
        reward_index: pool_info.reward_index,
    })
}

pub fn query_bond_amount(deps: Deps) -> StdResult<Uint128> {
    let bond_amount = TOTAL_BOND_AMOUNT.load(deps.storage)?;

    Ok(bond_amount)
}

fn validate_protocol_fee(fee: Decimal) -> StdResult<Decimal> {
    if fee > Decimal::from_str(MAX_PROTOCOL_FEE)? {
        return Err(StdError::generic_err(format!(
            "fee can not be greater than {}",
            MAX_PROTOCOL_FEE
        )));
    }

    Ok(fee)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    Ok(Response::default())
}
