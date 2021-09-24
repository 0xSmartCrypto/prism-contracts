use cosmwasm_std::{attr, to_binary, CosmosMsg, DepsMut, Response, StdResult, Uint128, WasmMsg};

use crate::rewards::pull_rewards;
use crate::state::{BOND_AMOUNTS, CONFIG, TOTAL_BOND_AMOUNT, BondInfo};
use cw20::Cw20ExecuteMsg;
use terra_cosmwasm::TerraMsgWrapper;

pub fn bond(
    deps: DepsMut,
    staker_addr: String,
    amount: Uint128,
    mode: Option<String>,
) -> StdResult<Response<TerraMsgWrapper>> {
    pull_rewards(deps.storage, &staker_addr)?;
    let bond_total = TOTAL_BOND_AMOUNT.load(deps.storage)?;
    TOTAL_BOND_AMOUNT.save(deps.storage, &(bond_total + amount))?;

    let mut b = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .unwrap_or(BondInfo { bond_amount: Uint128::zero(), mode: mode.clone() });
    // allow update of mode if nothing is bonded
    if b.bond_amount == Uint128::zero() {
        b.mode = mode;
    }
    b.bond_amount += amount;
    BOND_AMOUNTS.save(
        deps.storage,
        staker_addr.as_bytes(),
        &b,
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "bond"),
        attr("staker_addr", staker_addr.as_str()),
        attr("amount", amount.to_string()),
    ]))
}

pub fn unbond(
    deps: DepsMut,
    staker_addr: String,
    amount: Uint128,
) -> StdResult<Response<TerraMsgWrapper>> {
    pull_rewards(deps.storage, &staker_addr)?;
    let bond_total = TOTAL_BOND_AMOUNT.load(deps.storage)?;
    TOTAL_BOND_AMOUNT.save(deps.storage, &(bond_total - amount))?;

    let mut b = BOND_AMOUNTS.load(deps.storage, staker_addr.as_bytes())?;
    b.bond_amount -= amount;
    BOND_AMOUNTS.save(
        deps.storage,
        staker_addr.as_bytes(),
        &b,
    )?;

    let cfg = CONFIG.load(deps.storage)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token,
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: staker_addr.to_string(),
                amount,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            attr("action", "unbond"),
            attr("staker_addr", staker_addr.as_str()),
            attr("amount", amount.to_string()),
        ]))
}
