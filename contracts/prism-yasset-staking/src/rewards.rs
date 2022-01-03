use crate::contract::_query_state;
use crate::error::{ContractError, ContractResult};
use crate::state::{RewardInfo, BOND_AMOUNTS, CONFIG, POOL_INFO, REWARDS};
use cosmwasm_std::{
    attr, to_binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, QuerierWrapper,
    QueryRequest, Response, StdError, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};

use astroport::asset::{Asset, AssetInfo};
use cw20::Cw20ExecuteMsg;
use prism_protocol::reward_distribution::{
    QueryMsg as RewardDistributionQueryMsg, RewardAssetWhitelistResponse,
};
use prism_protocol::yasset_staking::RewardInfoResponse;

// deposit reward assets
pub fn deposit_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.reward_distribution_contract.clone().unwrap() {
        return Err(ContractError::Unauthorized {});
    }
    let state = _query_state(deps.as_ref(), &env, &cfg)?;

    // if we have nothing bonded, we shouldn't be receiving any rewards
    if state.total_bond_amount == Uint128::zero() {
        return Err(ContractError::ZeroBondedAmount {});
    }

    let mut messages = vec![];

    for asset in &assets {
        match asset.info.clone() {
            AssetInfo::NativeToken { .. } => {
                asset
                    .assert_sent_native_token_balance(&info)
                    .map_err(|_| ContractError::InvalidNativeFunds {})?;
            }
            AssetInfo::Token { contract_addr } => {
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: env.contract.address.to_string(),
                        amount: asset.amount,
                    })?,
                    funds: vec![],
                }));
            }
        }
        let mut pool_info = POOL_INFO
            .load(deps.storage, asset.info.to_string().as_bytes())
            .unwrap_or_default();

        let normal_reward_per_bond = Decimal::from_ratio(asset.amount, state.total_bond_amount);
        pool_info.reward_index = pool_info.reward_index + normal_reward_per_bond;
        POOL_INFO.save(deps.storage, asset.info.to_string().as_bytes(), &pool_info)?;
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "deposit_rewards")]))
}

// claim all available rewards
pub fn claim_rewards(deps: DepsMut, info: MessageInfo) -> ContractResult<Response> {
    let whitelisted_assets = query_whitelist(deps.storage, &deps.querier)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, info.sender.to_string().as_bytes())
        .map_err(|_| ContractError::InvalidUnbond {
            reason: "no tokens bonded".to_string(),
        })?;

    let mut messages = vec![];
    let mut attributes = vec![];
    for asset_info in whitelisted_assets {
        let mut reward_info = compute_asset_rewards(
            deps.storage,
            &info.sender.to_string(),
            bond_info.bond_amount,
            &asset_info,
        )?;

        // create the claim asset from the pending rewards, and reset pending to 0
        let claim_asset = Asset {
            info: asset_info.clone(),
            amount: reward_info.pending_reward,
        };
        reward_info.pending_reward = Uint128::zero();

        // save updated reward
        REWARDS.save(
            deps.storage,
            (info.sender.as_bytes(), asset_info.to_string().as_bytes()),
            &reward_info,
        )?;

        // if there is nothing to claim, skip
        if claim_asset.amount.is_zero() {
            continue;
        }

        let msg = claim_asset
            .clone()
            .into_msg(&deps.querier, info.sender.clone())?;
        messages.push(msg);
        attributes.push(attr("claimed_asset", format!("{}", &claim_asset)));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim_rewards")
        .add_attributes(attributes))
}

pub fn compute_asset_rewards(
    storage: &dyn Storage,
    staker: &String,
    bond_amount: Uint128,
    asset_info: &AssetInfo,
) -> StdResult<RewardInfo> {
    let pool_info = POOL_INFO
        .load(storage, asset_info.to_string().as_bytes())
        .unwrap_or_default();

    let mut reward_info: RewardInfo = match REWARDS.load(
        storage,
        (staker.as_bytes(), asset_info.to_string().as_bytes()),
    ) {
        Ok(mut info) => {
            let pending_reward = (bond_amount * pool_info.reward_index)
                .checked_sub(bond_amount * info.index)
                .map_err(StdError::from)?;

            info.pending_reward += pending_reward;

            info
        }
        Err(_) => RewardInfo::default(),
    };

    reward_info.index = pool_info.reward_index;
    Ok(reward_info)
}

pub fn compute_all_rewards(
    storage: &mut dyn Storage,
    querier: &QuerierWrapper,
    staker: &String,
    bond_amount: Uint128,
) -> StdResult<()> {
    let whitelisted_assets = query_whitelist(storage, querier)?;
    for asset in whitelisted_assets {
        let reward_info = compute_asset_rewards(storage, &staker, bond_amount, &asset)?;

        // save updated reward
        REWARDS.save(
            storage,
            (staker.as_bytes(), asset.to_string().as_bytes()),
            &reward_info,
        )?;
    }

    Ok(())
}

pub fn query_reward_info(deps: Deps, staker_addr: String) -> StdResult<RewardInfoResponse> {
    let whitelisted_assets = query_whitelist(deps.storage, &deps.querier)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .map_err(|_| StdError::generic_err("there is no reward info for this address"))?;

    // update all rewards
    let rewards = whitelisted_assets
        .iter()
        .map(|wlasset| {
            let reward_info =
                compute_asset_rewards(deps.storage, &staker_addr, bond_info.bond_amount, &wlasset)?;

            Ok(Asset {
                info: wlasset.clone(),
                amount: reward_info.pending_reward,
            })
        })
        .collect::<StdResult<Vec<Asset>>>()?;

    Ok(RewardInfoResponse {
        staker_addr,
        staked_amount: bond_info.bond_amount,
        rewards,
    })
}

pub fn query_whitelist(
    storage: &dyn Storage,
    querier: &QuerierWrapper,
) -> StdResult<Vec<AssetInfo>> {
    let cfg = CONFIG.load(storage)?;
    let res: RewardAssetWhitelistResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: cfg.reward_distribution_contract.unwrap().to_string(),
            msg: to_binary(&RewardDistributionQueryMsg::RewardAssetWhitelist {})?,
        }))?;

    Ok(res.assets)
}
