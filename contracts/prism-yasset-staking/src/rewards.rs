use cosmwasm_std::{
    attr, to_binary, BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::state::{
    RewardInfo, BOND_AMOUNTS, CONFIG, POOL_INFO, REWARDS, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS,
};

use astroport::asset::{Asset, AssetInfo};
use cw20::Cw20ExecuteMsg;
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::yasset_staking::{RewardInfoResponse, StakingMode};
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

    let mut messages = vec![];
    for asset in assets {
        if !whitelisted_assets.contains(&asset.info) {
            return Err(StdError::generic_err(format!(
                "asset {} is not whitelisted",
                asset.info.to_string()
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
                .load(deps.storage, &asset.info.to_string().as_bytes())
                .unwrap_or_default();

            let protocol_fee_amount = asset.amount * cfg.protocol_fee;
            let mut reward_amount = asset.amount - protocol_fee_amount;

            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: token_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: cfg.collector.to_string(),
                    amount: protocol_fee_amount,
                })?,
                funds: vec![],
            }));

            if total_bond_amount.is_zero() {
                pool_info.pending_reward += reward_amount;
            } else {
                reward_amount += pool_info.pending_reward;
                let normal_reward_per_bond = Decimal::from_ratio(reward_amount, total_bond_amount);

                pool_info.reward_index = pool_info.reward_index + normal_reward_per_bond;
                pool_info.pending_reward = Uint128::zero();
            }

            POOL_INFO.save(deps.storage, &asset.info.to_string().as_bytes(), &pool_info)?;
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
    let mut assets_to_swap: Vec<Asset> = vec![];
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

        if staking_mode == StakingMode::Default
            || asset_info.to_string() == cfg.prism_token.to_string()
        {
            // re-implement into_msg here because life is cruel
            // should never be native
            let msg = match &asset_info {
                AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: info.sender.to_string(),
                        amount: claim_asset.amount,
                    })?,
                    funds: vec![],
                }),
                AssetInfo::NativeToken { .. } => CosmosMsg::Bank(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: vec![claim_asset.deduct_tax(&deps.querier)?],
                }),
            };

            messages.push(msg);
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

            assets_to_swap.push(claim_asset.clone());
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
    staker: &String,
    bond_amount: Uint128,
    whitelisted_assets: &Vec<AssetInfo>,
) -> StdResult<()> {
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
        staking_mode: bond_info.mode,
        rewards,
    })
}
