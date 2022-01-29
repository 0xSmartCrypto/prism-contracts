use cosmwasm_std::{
    attr, to_binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};

use crate::querier::query_vault_bond_amount;
use crate::state::{
    RewardInfo, BOND_AMOUNTS, CONFIG, POOL_INFO, REWARDS, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS,
};

use cw20::Cw20ExecuteMsg;
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::yasset_staking::{RewardInfoResponse, StakingMode};
use prismswap::asset::{Asset, AssetInfo};
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
        if let AssetInfo::Token {
            contract_addr: token_addr,
            ..
        } = &asset.info
        {
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
                .load(deps.storage, asset.info.to_string().as_bytes())
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

                POOL_INFO.save(deps.storage, asset.info.to_string().as_bytes(), &pool_info)?;
            }
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "deposit_rewards")]))
}

// claim all available rewards
pub fn claim_rewards(deps: DepsMut, info: MessageInfo) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
    let bond_info = BOND_AMOUNTS
        .load(deps.storage, info.sender.to_string().as_bytes())
        .map_err(|_| StdError::generic_err("no tokens bonded"))?;

    let staking_mode = bond_info.mode.unwrap_or(StakingMode::Default);

    let mut messages = vec![];
    let mut attributes = vec![];
    let mut assets_to_swap: Vec<cw_asset::Asset> = vec![];
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

        if staking_mode == StakingMode::Default || asset_info.to_string() == cfg.prism_token {
            // re-implement into_msg here because life is cruel
            if let AssetInfo::Token { contract_addr } = asset_info {
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
                panic!("Native reward assets not supported");
            }
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: cfg.collector.to_string(),
                    amount: claim_asset.amount,
                    expires: None,
                })?,
                funds: vec![],
            }));

            assets_to_swap.push(claim_asset.clone().into());
        }

        attributes.push(attr("claimed_asset", format!("{}", &claim_asset)));
    }

    if !assets_to_swap.is_empty() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.collector.to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                receiver: Some(info.sender.to_string()),
                assets: assets_to_swap,
            })?,
            funds: vec![],
        }))
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim_rewards")
        .add_attributes(attributes))
}

pub fn compute_asset_rewards(
    storage: &dyn Storage,
    staker: &str,
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
        REWARDS.save(
            storage,
            (staker.as_bytes(), asset.to_string().as_bytes()),
            &reward_info,
        )?;
    }

    Ok(())
}

pub fn whitelist_reward_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by gov
    if info.sender.as_str() != cfg.gov.as_str() {
        return Err(StdError::generic_err("unauthorized"));
    }

    if asset.is_native_token() {
        return Err(StdError::generic_err("only token assets can be registered"));
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
    info: MessageInfo,
    asset: AssetInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by gov
    if info.sender.as_str() != cfg.gov.as_str() {
        return Err(StdError::generic_err("unauthorized"));
    }

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
        staking_mode: bond_info.mode,
        rewards,
    })
}
