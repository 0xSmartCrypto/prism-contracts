use cosmwasm_std::{
    attr, to_binary, BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::state::{
    PoolInfo, RewardInfo, BOND_AMOUNTS, POOL_INFO, REWARDS, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS,
};

use cw20::Cw20ExecuteMsg;
use terra_cosmwasm::TerraMsgWrapper;
use terraswap::asset::{Asset, AssetInfo};

// deposit_reward must be from reward token contract
pub fn deposit_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> StdResult<Response<TerraMsgWrapper>> {
    let mut messages: Vec<CosmosMsg<WasmMsg>> = vec![];
    let total_bond = TOTAL_BOND_AMOUNT.load(deps.storage)?;

    for asset in assets {
        if env.contract.address == info.sender {
        } else if let AssetInfo::Token { contract_addr, .. } = &asset.info {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: asset.amount,
                })?,
                funds: vec![],
            }));
        } else {
            return Err(StdError::generic_err("may not deposit native tokens"));
        }
        let mut pool_info = POOL_INFO
            .load(deps.storage, &asset.info.to_string().as_bytes())
            .unwrap_or(PoolInfo {
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });

        let mut reward_amount = asset.amount.clone();
        if total_bond.is_zero() {
            pool_info.pending_reward += reward_amount;
        } else {
            reward_amount += pool_info.pending_reward;
            let normal_reward_per_bond = Decimal::from_ratio(reward_amount, total_bond);
            pool_info.reward_index = pool_info.reward_index + normal_reward_per_bond;
            pool_info.pending_reward = Uint128::zero();
        }

        POOL_INFO.save(deps.storage, &asset.info.to_string().as_bytes(), &pool_info)?;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "deposit_reward")]))
}

// withdraw all rewards or single reward depending on asset_token
pub fn withdraw_reward(deps: DepsMut, info: MessageInfo) -> StdResult<Response<TerraMsgWrapper>> {
    let mut messages = vec![];
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;

    pull_rewards(deps.storage, &info.sender.clone().into_string())?;
    for asset_info in whitelisted_assets {
        let mut reward_info = REWARDS.load(
            deps.storage,
            (info.sender.as_bytes(), asset_info.to_string().as_bytes()),
        )?;

        let asset = Asset {
            info: asset_info.clone(),
            amount: reward_info.pending_reward,
        };

        // re-implement into_msg here because life is cruel
        let msg = match &asset_info {
            AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: asset.amount,
                })?,
                funds: vec![],
            }),
            AssetInfo::NativeToken { .. } => CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![asset.deduct_tax(&deps.querier)?],
            }),
        };

        messages.push(msg);

        reward_info.pending_reward = Uint128::zero();
        REWARDS.save(
            deps.storage,
            (info.sender.as_bytes(), asset_info.to_string().as_bytes()),
            &reward_info,
        )?;
    }
    Ok(Response::new().add_messages(messages))
}

// withdraw all rewards to pending rewards
pub fn pull_rewards(storage: &mut dyn Storage, owner: &String) -> StdResult<()> {
    let bond_amount = BOND_AMOUNTS
        .load(storage, owner.as_bytes())
        .unwrap_or(Uint128::zero());

    let whitelisted_assets = WHITELISTED_ASSETS.load(storage)?;
    for asset_info in whitelisted_assets {
        let pool_info = POOL_INFO
            .load(storage, asset_info.to_string().as_bytes())
            .unwrap_or(PoolInfo {
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });
        let mut reward_info = REWARDS
            .load(
                storage,
                (owner.as_bytes(), asset_info.to_string().as_bytes()),
            )
            .unwrap_or(RewardInfo {
                index: pool_info.reward_index,
                pending_reward: Uint128::zero(),
            });
        let pending_reward =
            (bond_amount * pool_info.reward_index).checked_sub(bond_amount * reward_info.index)?;
        reward_info.index = pool_info.reward_index;
        reward_info.pending_reward += pending_reward;
        REWARDS.save(
            storage,
            (&owner.as_bytes(), &asset_info.to_string().as_bytes()),
            &reward_info,
        )?
    }
    Ok(())
}

pub fn query_reward_info(deps: Deps, staker_addr: String) -> StdResult<Vec<Asset>> {
    let bond_amount = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .unwrap_or(Uint128::zero());

    let mut reward_infos = vec![];

    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
    for asset_info in whitelisted_assets {
        let pool_info = POOL_INFO
            .load(deps.storage, asset_info.to_string().as_bytes())
            .unwrap_or(PoolInfo {
                pending_reward: Uint128::zero(),
                reward_index: Decimal::zero(),
            });
        let mut reward_info = REWARDS
            .load(
                deps.storage,
                (staker_addr.as_bytes(), asset_info.to_string().as_bytes()),
            )
            .unwrap_or(RewardInfo {
                index: pool_info.reward_index,
                pending_reward: Uint128::zero(),
            });
        let pending_reward =
            (bond_amount * pool_info.reward_index).checked_sub(bond_amount * reward_info.index)?;
        reward_info.index = pool_info.reward_index;
        reward_info.pending_reward += pending_reward;
        reward_infos.push(Asset {
            info: asset_info.clone(),
            amount: reward_info.pending_reward,
        });
    }
    Ok(reward_infos)
}
