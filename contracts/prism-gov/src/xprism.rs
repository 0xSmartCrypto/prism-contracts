use crate::state::config_read;
use cosmwasm_std::{
    attr, to_binary, CosmosMsg, DepsMut, Env, Response, StdResult, Storage, Uint128, WasmMsg,
};
use cw20_base::msg::ExecuteMsg as TokenMsg;
use cw_storage_plus::{Bound, Map};
use prism_protocol::common::OrderBy;
use std::convert::TryInto;
use terraswap::querier::query_token_balance;

// map (address, return_date) -> (xprism_amt, prism_amt)
pub const PRISM_RETURN: Map<(&[u8], &[u8]), (Uint128, Uint128)> = Map::new("prism_return");
// address -> (xprism_amt, prism_amt)
// upon withdraw, prism_amt is returned, xprism_amt is burned
pub const PENDING_WITHDRAW: Map<&[u8], (Uint128, Uint128)> = Map::new("pending_withdraw");

// seconds in a day, make time discrete per day
pub const TIME_UNIT: u64 = 60 * 60 * 24;
pub const REDEMPTION_TIME: u64 = TIME_UNIT * 21u64;

pub fn mint_xprism(
    deps: DepsMut,
    env: Env,
    sender: String,
    amount: Uint128,
) -> StdResult<Response> {
    let cfg = config_read(deps.storage).load()?;
    let prism_token = deps.api.addr_humanize(&cfg.prism_token)?;
    let xprism_token = deps.api.addr_humanize(&cfg.xprism_token)?;

    let prism_amt = query_token_balance(&deps.querier, prism_token, env.contract.address.clone())?;
    let xprism_amt =
        query_token_balance(&deps.querier, xprism_token.clone(), env.contract.address)?;

    let xprism_to_mint = if xprism_amt.is_zero() {
        amount
    } else {
        amount.multiply_ratio(xprism_amt, prism_amt)
    };

    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: xprism_token.clone().into_string(),
            msg: to_binary(&TokenMsg::Mint {
                recipient: sender.clone(),
                amount: xprism_to_mint,
            })?,
            funds: vec![],
        })),
    )
}

pub fn pull_rewards(
    storage: &mut dyn Storage,
    current_time: u64,
    address: &String,
) -> StdResult<()> {
    let start = Some(Bound::Inclusive(address.as_bytes().to_vec()));
    let address_vec = address.as_bytes().to_vec();
    let address_len = address_vec.len();

    let (mut w_xprism, mut w_prism) = PENDING_WITHDRAW
        .load(storage, address.as_bytes())
        .unwrap_or((Uint128::zero(), Uint128::zero()));
    let mut to_delete = vec![];

    for item in PRISM_RETURN.range(storage, start, None, OrderBy::Asc.into()) {
        let (key, (xprism_amt, prism_amt)) = item?;
        let end_time = u64::from_be_bytes(key[address_len..].try_into().unwrap());
        if !key.starts_with(address_vec.as_slice()) || current_time < end_time {
            break;
        }
        w_xprism += xprism_amt;
        w_prism += prism_amt;
        to_delete.push(end_time);
    }

    for t in to_delete {
        PRISM_RETURN.remove(storage, (address.as_bytes(), &t.to_be_bytes()))
    }
    PENDING_WITHDRAW.save(storage, address.as_bytes(), &(w_xprism, w_prism))
}

pub fn redeem_xprism(
    deps: DepsMut,
    env: Env,
    sender: &String,
    amount: Uint128,
) -> StdResult<Response> {
    let cfg = config_read(deps.storage).load()?;
    let prism_token = deps.api.addr_humanize(&cfg.prism_token)?;
    let xprism_token = deps.api.addr_humanize(&cfg.xprism_token)?;

    pull_rewards(deps.storage, env.block.time.seconds(), &sender)?;

    let prism_amt = query_token_balance(
        &deps.querier,
        prism_token.clone(),
        env.contract.address.clone(),
    )?;
    let xprism_amt =
        query_token_balance(&deps.querier, xprism_token.clone(), env.contract.address)?;

    let prism_to_return = amount.multiply_ratio(prism_amt, xprism_amt);

    let mut end_time = env.block.time.seconds() + REDEMPTION_TIME;
    end_time -= end_time % TIME_UNIT;

    let (orig_xprism, orig_prism) = PRISM_RETURN
        .load(deps.storage, (sender.as_bytes(), &end_time.to_be_bytes()))
        .unwrap_or((Uint128::zero(), Uint128::zero()));

    PRISM_RETURN.save(
        deps.storage,
        (sender.as_bytes(), &end_time.to_be_bytes()),
        &(orig_xprism + amount, orig_prism + prism_to_return),
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "redeem_xprism"),
        attr("total_redeemed", amount.to_string()),
        attr("prism_queued", prism_to_return.to_string()),
    ]))
}

pub fn claim_redeemed_prism(deps: DepsMut, env: Env, sender: &String) -> StdResult<Response> {
    let cfg = config_read(deps.storage).load()?;
    let prism_token = deps.api.addr_humanize(&cfg.prism_token)?;
    let xprism_token = deps.api.addr_humanize(&cfg.xprism_token)?;

    pull_rewards(deps.storage, env.block.time.seconds(), &sender)?;
    let (w_xprism, w_prism) = PENDING_WITHDRAW.load(deps.storage, sender.as_bytes())?;
    PENDING_WITHDRAW.save(
        deps.storage,
        sender.as_bytes(),
        &(Uint128::zero(), Uint128::zero()),
    )?;

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: xprism_token.clone().into_string(),
            msg: to_binary(&TokenMsg::Burn { amount: w_xprism })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: prism_token.clone().into_string(),
            msg: to_binary(&TokenMsg::Transfer {
                recipient: sender.clone(),
                amount: w_prism,
            })?,
            funds: vec![],
        }),
    ]))
}
