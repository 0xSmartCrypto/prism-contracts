use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};

use crate::querier::query_vault_bond_amount;
use crate::state::{
    Config, RewardInfo, BOND_AMOUNTS, CONFIG, POOL_INFO, REWARDS, TOTAL_BOND_AMOUNT,
    WHITELISTED_ASSETS,
};

use cw20::Cw20ExecuteMsg;
use cw_asset::{Asset, AssetInfo};
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::gov::Cw20HookMsg as GovCw20HookMsg;
use prism_protocol::yasset_staking::{ExecuteMsg, RewardInfoResponse};
use prismswap::asset::PrismSwapAssetInfo;
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
    let total_bond_amount = TOTAL_BOND_AMOUNT.load(deps.storage)?;
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;

    let vault_bond_amount = query_vault_bond_amount(&deps.querier, cfg.vault)?;
    // if total_bond_amount is zero it means that all yLuna is circulating and not staked
    // vault_bond amount can only be zero if total_bond_amount is also zero (no yLuna can exist with not luna on vault)
    let stakers_portion = if total_bond_amount.is_zero() {
        Decimal::zero()
    } else {
        Decimal::from_ratio(total_bond_amount, vault_bond_amount).min(Decimal::one())
    };

    let mut messages = vec![];
    for asset in assets {
        if !whitelisted_assets.contains(&asset.info) {
            return Err(StdError::generic_err(format!(
                "asset {} is not whitelisted",
                asset.info
            )));
        }

        // no need to handle native tokens, because native tokens can not be whitelisted
        if let AssetInfo::Cw20(token_addr) = &asset.info {
            if env.contract.address != info.sender {
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: token_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: env.contract.address.to_string(),
                        amount: asset.amount,
                    })?,
                    funds: vec![],
                }));
            }

            let mut pool_info = POOL_INFO
                .load(deps.storage, asset.info.as_bytes())
                .unwrap_or_default();

            let stakers_portion_amount = asset.amount * stakers_portion;
            let protocol_fee_amount = stakers_portion_amount * cfg.protocol_fee;
            let reward_amount = stakers_portion_amount.checked_sub(protocol_fee_amount)?;

            // send the difference to collector
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: token_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: cfg.collector.to_string(),
                    amount: asset.amount.checked_sub(reward_amount)?,
                })?,
                funds: vec![],
            }));

            if !total_bond_amount.is_zero() {
                let normal_reward_per_bond = Decimal::from_ratio(reward_amount, total_bond_amount);
                pool_info.reward_index = pool_info.reward_index + normal_reward_per_bond;

                POOL_INFO.save(deps.storage, asset.info.as_bytes(), &pool_info)?;
            }
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "deposit_rewards")]))
}

// claim all available rewards
pub fn claim_rewards(deps: DepsMut, info: MessageInfo) -> StdResult<Response<TerraMsgWrapper>> {
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
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
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, info.sender.to_string().as_bytes())
        .map_err(|_| StdError::generic_err("no tokens bonded"))?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    let mut attributes = vec![];
    let mut swap_assets: Vec<Asset> = vec![];

    // verify that the claim asset is supported, and return the underlying Addr
    let claim_token = verify_claim_asset(&cfg, &claim_asset_info)?;

    // for xprism claim token, we first swap to prism and then mint xprism with gov
    // as a second step.  so if claim token is xprism, we swap to prism using
    // contract address as receiver.  otherwise we swap to claim token using
    // sender/claimer as receiver
    let (swap_dest_asset_info, swap_receiver) = if claim_token == cfg.xprism_token {
        (
            AssetInfo::Cw20(cfg.prism_token.clone()),
            env.contract.address.clone(),
        )
    } else {
        (AssetInfo::Cw20(claim_token.clone()), info.sender.clone())
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
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: claim_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: reward_asset.amount,
                })?,
                funds: vec![],
            }));
            continue;
        };

        // if this reward asset is already in swap denom, nothing to do, continue.
        // the only way this can happen is if prism becomes a reward asset
        if reward_asset.info == swap_dest_asset_info {
            continue;
        };

        // increase allowance for the collector
        if let AssetInfo::Cw20(contract_addr) = &asset_info {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: cfg.collector.to_string(),
                    amount: reward_asset.amount,
                    expires: None,
                })?,
                funds: vec![],
            }));
        } else {
            // this is a logic error in the code, native reward assets not allowed
            return Err(StdError::generic_err("Native reward assets not supported"));
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
            funds: vec![],
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
    env: Env,
    cfg: Config,
    receiver: Addr,
    prev_balance: Uint128,
) -> StdResult<Response<TerraMsgWrapper>> {
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

pub fn whitelist_reward_asset(
    deps: DepsMut,
    asset: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    if asset.is_native_token() {
        return Err(StdError::generic_err(
            "only token assets can be whitelisted",
        ));
    }

    let mut whitelist = WHITELISTED_ASSETS.load(deps.storage)?;
    whitelist.push(asset.clone());

    WHITELISTED_ASSETS.save(deps.storage, &whitelist)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "whitelist_reward_asset"),
        attr("whitelisted_asset", asset.to_string()),
    ]))
}

pub fn remove_whitelisted_reward_asset(
    deps: DepsMut,
    asset: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let mut whitelist = WHITELISTED_ASSETS.load(deps.storage)?;

    match whitelist.iter().position(|item| item.eq(&asset)) {
        Some(position) => {
            whitelist.remove(position);
        }
        None => return Err(StdError::generic_err("this asset is not whitelisted")),
    }

    WHITELISTED_ASSETS.save(deps.storage, &whitelist)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "remove_whitelisted_reward_asset"),
        attr("removed_asset", asset.to_string()),
    ]))
}

pub fn query_reward_info(deps: Deps, staker_addr: String) -> StdResult<RewardInfoResponse> {
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .map_err(|_| StdError::generic_err("there is no reward info for this address"))?;

    // update all rewards
    let rewards = whitelisted_assets
        .iter()
        .map(|wlasset| {
            let reward_info =
                compute_asset_rewards(deps.storage, &staker_addr, bond_info.bond_amount, wlasset)?;

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

fn verify_claim_asset(cfg: &Config, claim_asset_info: &AssetInfo) -> StdResult<Addr> {
    if let AssetInfo::Cw20(claim_addr) = claim_asset_info.clone() {
        if claim_addr == cfg.prism_token
            || claim_addr == cfg.xprism_token
            || claim_addr == cfg.cluna_token
            || claim_addr == cfg.yluna_token
            || claim_addr == cfg.pluna_token
        {
            return Ok(claim_addr);
        } else {
            return Err(StdError::generic_err("Claim asset not supported"));
        }
    }
    Err(StdError::generic_err("Native claim assets not supported"))
}
