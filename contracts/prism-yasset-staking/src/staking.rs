use cosmwasm_std::{
    attr, to_binary, CosmosMsg, DepsMut, MessageInfo, Response, StdError, StdResult, Uint128,
    WasmMsg,
};

use crate::rewards::compute_all_rewards;
use crate::state::{BOND_AMOUNTS, CONFIG, TOTAL_BOND_AMOUNT, WHITELISTED_ASSETS};
use cw20::Cw20ExecuteMsg;
use terra_cosmwasm::TerraMsgWrapper;

pub fn bond(
    deps: DepsMut,
    staker_addr: String, // address of person sending their y-asset
    amount: Uint128,     // amount of y-asset they are sending to be staked
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

    // update bond amount
    bond_info.bond_amount += amount;
    TOTAL_BOND_AMOUNT.save(deps.storage, &(bond_total + amount))?;
    BOND_AMOUNTS.save(deps.storage, staker_addr.as_bytes(), &bond_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "bond"),
        attr("staker_addr", staker_addr.as_str()),
        attr("amount", amount.to_string()),
    ]))
}

pub fn unbond(
    deps: DepsMut,
    info: MessageInfo,
    amount: Option<Uint128>, // If None, the user's entire stake will be unstaked.
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
        // Unbond everything if input amount is not specified.
        None => bond_info.bond_amount,
    };

    if unbonded_amt > bond_total {
        // Theoretically impossible unless we have a bug or someone tampers with BOND_AMOUNTS.
        return Err(StdError::generic_err(
            "can not unbond more than total bonded amount",
        ));
    }

    // update state, user bond amount and total
    bond_info.bond_amount -= unbonded_amt;
    TOTAL_BOND_AMOUNT.save(deps.storage, &(bond_total - unbonded_amt))?;
    BOND_AMOUNTS.save(deps.storage, staker_addr.as_bytes(), &bond_info)?;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yluna_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: staker_addr.to_string(),
                amount: unbonded_amt,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "unbond"),
            attr("staker_addr", staker_addr.as_str()),
            attr("amount", unbonded_amt.to_string()),
        ]))
}
