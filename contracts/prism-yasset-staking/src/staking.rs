use cosmwasm_std::{
    attr, to_binary, CosmosMsg, DepsMut, MessageInfo, Response, StdError, StdResult, Uint128,
    WasmMsg,
};
use prism_protocol::yasset_staking::StakingMode;

use crate::rewards::compute_all_rewards;
use crate::state::{BOND_AMOUNTS, CONFIG, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS};
use cw20::Cw20ExecuteMsg;
use terra_cosmwasm::TerraMsgWrapper;

pub fn bond(
    deps: DepsMut,
    staker_addr: String,
    amount: Uint128,
    mode: Option<StakingMode>,
) -> StdResult<Response<TerraMsgWrapper>> {
    let bond_total = TOTAL_BOND_AMOUNT.load(deps.storage)?;
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
    let mut bond_info = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .unwrap_or_default();

    // update reward pools
    compute_all_rewards(
        deps.storage,
        &staker_addr.to_string(),
        bond_info.bond_amount,
        &whitelisted_assets,
    )?;

    // allow update of mode if nothing is bonded
    if bond_info.bond_amount == Uint128::zero() {
        bond_info.mode = mode.clone();
    } else if mode.is_some() {
        return Err(StdError::generic_err(
            "mode can only be changed if nothing is bonded",
        ));
    }

    // update bond amount
    bond_info.bond_amount += amount;
    TOTAL_BOND_AMOUNT.save(deps.storage, &(bond_total + amount))?;
    BOND_AMOUNTS.save(deps.storage, staker_addr.as_bytes(), &bond_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "bond"),
        attr("staker_addr", staker_addr.as_str()),
        attr("amount", amount.to_string()),
        attr("mode", mode.unwrap_or(StakingMode::Default).to_string()),
    ]))
}

pub fn unbond(
    deps: DepsMut,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> StdResult<Response<TerraMsgWrapper>> {
    let staker_addr = info.sender.to_string();
    let cfg = CONFIG.load(deps.storage)?;
    let bond_total = TOTAL_BOND_AMOUNT.load(deps.storage)?;
    let whitelisted_assets = WHITELISTED_ASSETS.load(deps.storage)?;
    let mut bond_info = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .map_err(|_| StdError::generic_err("no tokens bonded"))?;

    // update reward pools
    compute_all_rewards(
        deps.storage,
        &staker_addr.to_string(),
        bond_info.bond_amount,
        &whitelisted_assets,
    )?;

    let unbonded_amt = match amount {
        Some(amount) => {
            if amount > bond_info.bond_amount {
                return Err(StdError::generic_err(
                    "can not unbond more than the bonded amount",
                ));
            }

            amount
        }
        None => bond_info.bond_amount,
    };

    // update state, user bond amount and total
    bond_info.bond_amount -= unbonded_amt;
    TOTAL_BOND_AMOUNT.save(deps.storage, &(bond_total - unbonded_amt))?;
    BOND_AMOUNTS.save(deps.storage, staker_addr.as_bytes(), &bond_info)?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];

    let staking_mode = bond_info.mode.unwrap_or(StakingMode::Default);
    let withdraw_fee: Uint128 = if staking_mode == StakingMode::XPrism {
        let fee: Uint128 = unbonded_amt * cfg.withdraw_fee;
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: staker_addr.to_string(),
                amount: unbonded_amt,
            })?,
            funds: vec![],
        }));

        fee
    } else {
        Uint128::zero()
    };

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.yluna_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: staker_addr.to_string(),
            amount: unbonded_amt - withdraw_fee,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "unbond"),
        attr("staker_addr", staker_addr.as_str()),
        attr("amount", unbonded_amt.to_string()),
        attr("withdraw_fee", withdraw_fee.to_string()),
    ]))
}
