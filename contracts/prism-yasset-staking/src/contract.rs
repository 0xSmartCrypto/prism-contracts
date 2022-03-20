#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, from_binary, to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};

use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    StateResponse,
};

use crate::rewards::{
    claim_rewards, convert_and_claim_rewards, deposit_rewards, mint_xprism_claim_hook,
    query_reward_info,
};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG, POOL_INFO, TOTAL_BOND_AMOUNT};

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

    CONFIG.save(
        deps.storage,
        &Config {
            owner: deps.api.addr_validate(&msg.owner)?,
            gov: deps.api.addr_validate(&msg.gov)?,
            collector: deps.api.addr_validate(&msg.collector)?,
            yasset_token: deps.api.addr_validate(&msg.yasset_token)?,
            prism_token: deps.api.addr_validate(&msg.prism_token)?,
            xprism_token: deps.api.addr_validate(&msg.xprism_token)?,
            reward_distribution: deps.api.addr_validate(&msg.reward_distribution)?,
            claim_assets: msg.claim_assets,
        },
    )?;

    TOTAL_BOND_AMOUNT.save(deps.storage, &Uint128::zero())?;
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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg), // Bond
        ExecuteMsg::Unbond { amount } => unbond(deps, info, amount),
        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, info),
        ExecuteMsg::ConvertAndClaimRewards { claim_asset } => {
            claim_asset.check(deps.api)?;
            convert_and_claim_rewards(deps, env, info, claim_asset)
        }
        ExecuteMsg::MintXprismClaimHook {
            receiver,
            prev_balance,
        } => mint_xprism_claim_hook(deps, info, env, receiver, prev_balance),
        ExecuteMsg::DepositRewards { assets } => {
            for asset in &assets {
                asset.info.check(deps.api)?;
            }
            deposit_rewards(deps, env, info, assets)
        }
        ExecuteMsg::UpdateConfig { owner } => update_config(deps, info, owner),
        ExecuteMsg::AddClaimAsset { asset } => {
            asset.check(deps.api)?;
            add_claim_asset(deps, info, asset)
        }
        ExecuteMsg::RemoveClaimAsset { asset } => {
            asset.check(deps.api)?;
            remove_claim_asset(deps, info, asset)
        }
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
            if cfg.yasset_token != info.sender {
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
) -> StdResult<Response<TerraMsgWrapper>> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by owner
    if info.sender != cfg.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        cfg.owner = deps.api.addr_validate(&owner)?;
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
        QueryMsg::RewardInfo { staker_addr } => to_binary(&query_reward_info(deps, staker_addr)?),
        QueryMsg::BondAmount {} => to_binary(&query_bond_amount(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        gov: cfg.gov.to_string(),
        collector: cfg.collector.to_string(),
        yasset_token: cfg.yasset_token.to_string(),
        prism_token: cfg.prism_token.to_string(),
        xprism_token: cfg.xprism_token.to_string(),
        reward_distribution: cfg.reward_distribution.to_string(),
        claim_assets: cfg.claim_assets,
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

pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let res = StateResponse {
        total_bond_amount: query_bond_amount(deps)?,
    };
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    Ok(Response::default())
}

pub fn add_claim_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by owner
    if info.sender != cfg.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if cfg.claim_assets.contains(&asset) {
        return Err(StdError::generic_err("duplicate claim asset"));
    }
    cfg.claim_assets.push(asset.clone());

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "add_claim_asset"),
        attr("claim_asset", asset.to_string()),
    ]))
}

pub fn remove_claim_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // can only be executed by owner
    if info.sender != cfg.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    match cfg.claim_assets.iter().position(|item| item.eq(&asset)) {
        Some(position) => {
            cfg.claim_assets.remove(position);
        }
        None => return Err(StdError::generic_err("claim asset doesn't exist")),
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "remove_claim_asset"),
        attr("removed_asset", asset.to_string()),
    ]))
}
