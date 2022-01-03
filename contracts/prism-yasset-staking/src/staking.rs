use cosmwasm_std::{attr, to_binary, CosmosMsg, DepsMut, MessageInfo, Response, Uint128, WasmMsg};

use crate::error::{ContractError, ContractResult};
use crate::rewards::compute_all_rewards;
use crate::state::{BOND_AMOUNTS, CONFIG};

use cw20::Cw20ExecuteMsg;

pub fn bond(deps: DepsMut, staker_addr: String, amount: Uint128) -> ContractResult<Response> {
    let mut bond_info = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .unwrap_or_default();

    // update reward pools
    compute_all_rewards(
        deps.storage,
        &deps.querier,
        &staker_addr.to_string(),
        bond_info.bond_amount,
    )?;

    // update user bond amount
    bond_info.bond_amount += amount;
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
    amount: Option<Uint128>,
) -> ContractResult<Response> {
    let staker_addr = info.sender.to_string();
    let cfg = CONFIG.load(deps.storage)?;
    let mut bond_info = BOND_AMOUNTS
        .load(deps.storage, staker_addr.as_bytes())
        .map_err(|_| ContractError::InvalidUnbond {
            reason: "no tokens bonded".to_string(),
        })?;

    // update reward pools
    compute_all_rewards(
        deps.storage,
        &deps.querier,
        &staker_addr.to_string(),
        bond_info.bond_amount,
    )?;

    let unbonded_amt = match amount {
        Some(amount) => {
            if amount > bond_info.bond_amount {
                return Err(ContractError::InvalidUnbond {
                    reason: "can not unbond more than the bonded amount".to_string(),
                });
            }

            amount
        }
        None => bond_info.bond_amount,
    };

    // update user bond amount
    bond_info.bond_amount -= unbonded_amt;
    BOND_AMOUNTS.save(deps.storage, staker_addr.as_bytes(), &bond_info)?;

    let mut messages = vec![];

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.yasset_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: staker_addr.to_string(),
            amount: unbonded_amt,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "unbond"),
        attr("staker_addr", staker_addr.as_str()),
        attr("amount", unbonded_amt.to_string()),
    ]))
}
