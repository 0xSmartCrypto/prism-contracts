use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    QuerierWrapper, QueryRequest, Response, StdError, StdResult, Storage, Uint128, WasmMsg,
    WasmQuery,
};

use crate::state::{RewardInfo, BOND_AMOUNTS, CONFIG, POOL_INFO, REWARDS, TOTAL_BOND_AMOUNT};

use cw20::Cw20ExecuteMsg;
use cw_asset::{Asset, AssetInfo};
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::gov::Cw20HookMsg as GovCw20HookMsg;
use prism_protocol::reward_distribution::{
    QueryMsg as RewardDistributionQueryMsg, RewardAssetWhitelistResponse,
};
use prism_protocol::yasset_staking::{ExecuteMsg, RewardInfoResponse};
use prismswap::asset::{PrismSwapAsset, PrismSwapAssetInfo};
use prismswap::querier::query_token_balance;
use terra_cosmwasm::TerraMsgWrapper;

// deposit whitelisted reward assets
pub fn deposit_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    // only reward distribution contract can call this
    if info.sender != cfg.reward_distribution {
        return Err(StdError::generic_err("unauthorized"));
    }

    // if we have nothing bonded, we shouldn't be receiving any rewards
    let total_bond_amount = TOTAL_BOND_AMOUNT.load(deps.storage)?;
    if total_bond_amount == Uint128::zero() {
        return Err(StdError::generic_err("zero bonded amount"));
    }

    let mut messages = vec![];
    for asset in assets {
        let pool_key = match &asset.info {
            AssetInfo::Cw20(token_addr) => {
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: token_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: env.contract.address.to_string(),
                        amount: asset.amount,
                    })?,
                    funds: vec![],
                }));
                token_addr.to_string()
            }
            AssetInfo::Native(denom) => {
                asset.assert_sent_native_token_balance(&info)?;
                denom.clone()
            }
        };

        let mut pool_info = POOL_INFO
            .load(deps.storage, pool_key.as_bytes())
            .unwrap_or_default();

        let normal_reward_per_bond = Decimal::from_ratio(asset.amount, total_bond_amount);
        pool_info.reward_index = pool_info.reward_index + normal_reward_per_bond;
        POOL_INFO.save(deps.storage, pool_key.as_bytes(), &pool_info)?;
        println!("deposited {}", asset);
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "deposit_rewards")]))
}

// claim all available rewards
pub fn claim_rewards(deps: DepsMut, info: MessageInfo) -> StdResult<Response<TerraMsgWrapper>> {
    let whitelisted_assets = query_whitelist(deps.storage, &deps.querier)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, info.sender.to_string().as_bytes())
        .map_err(|_| StdError::generic_err("no tokens bonded"))?;

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
            (info.sender.as_bytes(), asset_info.as_bytes()),
            &reward_info,
        )?;

        // if there is nothing to claim, skip
        if claim_asset.amount.is_zero() {
            continue;
        }

        messages.push(get_transfer_msg(&claim_asset, &info.sender)?);
        /*
        if let AssetInfo::Cw20(contract_addr) = asset_info {
            // re-implement into_msg here because life is cruel
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: claim_asset.amount,
                })?,
                funds: vec![],
            }))
        } else {
            // this is a logic error in the code, native reward assets not allowed
            return Err(StdError::generic_err("Native reward assets not supported"));
        }
        */

        attributes.push(attr("claimed_asset", format!("{}", &claim_asset)));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim_rewards")
        .add_attributes(attributes))
}

/// convert all rewards into claim_asset_info and then claim those rewards. this
/// method uses the collector's ConvertAndSend logic to perform the swaps.  if
/// the claim asset is xprism, then we convert to prism and issue a
/// MintXprismClaimHook which mints the prism obtained from the CollectAndSend.
pub fn convert_and_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    claim_asset_info: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    let whitelisted_assets = query_whitelist(deps.storage, &deps.querier)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, info.sender.to_string().as_bytes())
        .map_err(|_| StdError::generic_err("no tokens bonded"))?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    let mut attributes = vec![];
    let mut swap_assets: Vec<Asset> = vec![];
    let mut funds: Vec<Coin> = vec![];

    // verify that the claim asset is supported
    if !cfg.claim_assets.contains(&claim_asset_info) {
        return Err(StdError::generic_err(format!(
            "claim asset not supported: {}",
            claim_asset_info
        )));
    }

    // for xprism claim token, we first swap to prism and then mint xprism with gov
    // as a second step.  so if claim token is xprism, we swap to prism using
    // contract address as receiver.  otherwise we swap to claim token using
    // sender/claimer as receiver
    let (swap_dest_asset_info, swap_receiver) = match claim_asset_info.clone() {
        AssetInfo::Cw20(addr) if addr == cfg.xprism_token => (
            AssetInfo::Cw20(cfg.prism_token.clone()),
            env.contract.address.clone(),
        ),
        _ => (claim_asset_info.clone(), info.sender.clone()),
    };

    for asset_info in whitelisted_assets {
        let mut reward_info = compute_asset_rewards(
            deps.storage,
            &info.sender.to_string(),
            bond_info.bond_amount,
            &asset_info,
        )?;

        if reward_info.pending_reward.is_zero() {
            continue;
        }

        // create the reward asset from the pending rewards, and reset pending to 0
        let reward_asset = Asset {
            info: asset_info.clone(),
            amount: reward_info.pending_reward,
        };
        reward_info.pending_reward = Uint128::zero();

        // save updated reward
        REWARDS.save(
            deps.storage,
            (info.sender.as_bytes(), asset_info.as_bytes()),
            &reward_info,
        )?;

        attributes.push(attr("claimed_asset", format!("{}", &reward_asset)));

        // if this asset is already in claim denom, send directly and continue
        if reward_asset.info == claim_asset_info {
            messages.push(get_transfer_msg(&reward_asset, &info.sender)?);
            continue;
        };

        // if this reward asset is already in swap denom, nothing to do, continue.
        // the only way this can happen is if prism becomes a reward asset
        if reward_asset.info == swap_dest_asset_info {
            continue;
        };

        // increase allowance for the collector or add to funds
        match reward_asset.info.clone() {
            AssetInfo::Cw20(contract_addr) => {
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                        spender: cfg.collector.to_string(),
                        amount: reward_asset.amount,
                        expires: None,
                    })?,
                    funds: vec![],
                }));
            }
            AssetInfo::Native(denom) => funds.push(Coin {
                denom: denom.clone(),
                amount: reward_asset.amount,
            }),
        }
        // add reward asset to swap assets
        swap_assets.push(reward_asset);
    }

    if !swap_assets.is_empty() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.collector.to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: swap_assets,
                receiver: Some(swap_receiver.to_string()),
                dest_asset_info: swap_dest_asset_info,
            })?,
            funds,
        }));

        // if we're the receiver, this means we need the mint xprism claim hook
        if swap_receiver == env.contract.address {
            let prism_balance =
                query_token_balance(&deps.querier, &cfg.prism_token, &env.contract.address)?;

            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::MintXprismClaimHook {
                    receiver: info.sender,
                    prev_balance: prism_balance,
                })?,
                funds: vec![],
            }));
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim_rewards")
        .add_attributes(attributes))
}

pub fn mint_xprism_claim_hook(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    receiver: Addr,
    prev_balance: Uint128,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    // there's no reason for anyone else to call this
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("unauthorized"));
    }

    // query our prism balance
    let prism_balance =
        query_token_balance(&deps.querier, &cfg.prism_token, &env.contract.address)?;

    // mint our current balance minus prev balance
    let mint_amount = prism_balance.checked_sub(prev_balance)?;

    // send prism balance to gov contract and issue a MintXprism call with
    // receiver specified appropriately to the user who initiated the
    // claim_and_convert_rewards method
    let res = Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.prism_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: cfg.gov.to_string(),
                amount: mint_amount,
                msg: to_binary(&GovCw20HookMsg::MintXprism {
                    receiver: Some(receiver.to_string()),
                })?,
            })?,
            funds: vec![],
        })])
        .add_attribute("action", "mint_xprism_claim_hook")
        .add_attribute("prism_amount_to_mint_xprism", mint_amount);
    Ok(res)
}

pub fn compute_asset_rewards(
    storage: &dyn Storage,
    staker: &str,
    bond_amount: Uint128,
    asset_info: &AssetInfo,
) -> StdResult<RewardInfo> {
    let pool_info = POOL_INFO
        .load(storage, asset_info.as_bytes())
        .unwrap_or_default();

    let mut reward_info: RewardInfo =
        match REWARDS.load(storage, (staker.as_bytes(), asset_info.as_bytes())) {
            Ok(mut info) => {
                let pending_reward =
                    (bond_amount * pool_info.reward_index).checked_sub(bond_amount * info.index)?;

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
    staker: &str,
    bond_amount: Uint128,
    whitelisted_assets: &[AssetInfo],
) -> StdResult<()> {
    for asset in whitelisted_assets {
        let reward_info = compute_asset_rewards(storage, staker, bond_amount, asset)?;

        // save updated reward
        REWARDS.save(storage, (staker.as_bytes(), asset.as_bytes()), &reward_info)?;
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
        .filter_map(|wlasset| {
            let reward_info =
                compute_asset_rewards(deps.storage, &staker_addr, bond_info.bond_amount, wlasset)
                    .unwrap();
            if reward_info.pending_reward != Uint128::zero() {
                Some(Asset {
                    info: wlasset.clone(),
                    amount: reward_info.pending_reward,
                })
            } else {
                None
            }
        })
        .collect();

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
            contract_addr: cfg.reward_distribution.to_string(),
            msg: to_binary(&RewardDistributionQueryMsg::RewardAssetWhitelist {})?,
        }))?;

    Ok(res.assets)
}

pub fn get_transfer_msg(asset: &Asset, to: &Addr) -> StdResult<CosmosMsg<TerraMsgWrapper>> {
    match &asset.info {
        AssetInfo::Cw20(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.into(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: to.into(),
                amount: asset.amount,
            })?,
            funds: vec![],
        })),
        AssetInfo::Native(denom) => Ok(CosmosMsg::Bank(BankMsg::Send {
            to_address: to.into(),
            amount: vec![Coin {
                denom: denom.clone(),
                amount: asset.amount,
            }],
        })),
    }
}
