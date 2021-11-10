use crate::state::{calc_range_end, calc_range_start, config_read, DEFAULT_LIMIT, MAX_LIMIT};
use astroport::querier::{query_supply, query_token_balance};
use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_storage_plus::{Bound, Map};
use prism_protocol::{common::OrderBy, gov::PrismWithdrawOrdersResponse};
use std::convert::TryInto;

// map (address, return_date) -> (xprism_amt, prism_amt)
pub const WITHDRAW_ORDERS: Map<(&[u8], &[u8]), (Uint128, Uint128)> = Map::new("withdraw_orders");

const MAX_ORDER_WITHDRAW_PER_TX: usize = 50usize;

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
    let xprism_amt = query_supply(&deps.querier, xprism_token.clone())?;

    let xprism_to_mint = if xprism_amt.is_zero() {
        amount
    } else {
        amount.multiply_ratio(xprism_amt, prism_amt)
    };

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: xprism_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender,
                amount: xprism_to_mint,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "mint_xprism"),
            attr("mint_amount", xprism_to_mint.to_string()),
        ]))
}

pub fn redeem_xprism(
    deps: DepsMut,
    env: Env,
    sender: String,
    amount: Uint128,
) -> StdResult<Response> {
    let cfg = config_read(deps.storage).load()?;
    let prism_token = deps.api.addr_humanize(&cfg.prism_token)?;
    let xprism_token = deps.api.addr_humanize(&cfg.xprism_token)?;

    let prism_amt = query_token_balance(&deps.querier, prism_token, env.contract.address.clone())?;
    let xprism_amt = query_supply(&deps.querier, xprism_token.clone())?;

    let prism_to_return = amount.multiply_ratio(prism_amt, xprism_amt);

    let end_time = env.block.time.plus_seconds(cfg.redemption_time).seconds();

    WITHDRAW_ORDERS.save(
        deps.storage,
        (sender.as_bytes(), &end_time.to_be_bytes()),
        &(amount, prism_to_return),
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "redeem_xprism"),
        attr("total_redeemed", amount.to_string()),
        attr("prism_queued", prism_to_return.to_string()),
    ]))
}

pub fn claim_redeemed_prism(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let cfg = config_read(deps.storage).load()?;
    let prism_token = deps.api.addr_humanize(&cfg.prism_token)?;
    let xprism_token = deps.api.addr_humanize(&cfg.xprism_token)?;

    let (w_xprism, w_prism) =
        compute_withdrawable(deps.storage, env.block.time.seconds(), &info.sender)?;

    if w_prism.is_zero() && w_xprism.is_zero() {
        return Err(StdError::generic_err("nothing to claim"));
    }

    Ok(Response::new()
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: xprism_token.clone().into_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn { amount: w_xprism })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: prism_token.clone().into_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: w_prism,
                })?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![
            attr("action", "claim_redeemed_prism"),
            attr("prism_claimed", w_prism.to_string()),
            attr("xprism_burned", w_xprism.to_string()),
        ]))
}

fn compute_withdrawable(
    storage: &mut dyn Storage,
    current_time: u64,
    address: &Addr,
) -> StdResult<(Uint128, Uint128)> {
    let (mut w_xprism, mut w_prism) = (Uint128::zero(), Uint128::zero());
    let mut to_delete = vec![];

    for item in WITHDRAW_ORDERS
        .prefix(address.as_bytes())
        .range(storage, None, None, Order::Ascending)
        .take(MAX_ORDER_WITHDRAW_PER_TX)
    {
        let (key, (xprism_amt, prism_amt)) = item?;

        let end_time = u64::from_be_bytes(key.try_into().unwrap());
        if current_time < end_time {
            break;
        }
        w_xprism += xprism_amt;
        w_prism += prism_amt;
        to_delete.push(end_time);
    }

    for t in to_delete {
        WITHDRAW_ORDERS.remove(storage, (address.as_bytes(), &t.to_be_bytes()))
    }

    Ok((w_xprism, w_prism))
}

pub fn query_prism_withdraw_orders(
    deps: Deps,
    env: Env,
    address: String,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PrismWithdrawOrdersResponse> {
    let address: Addr = deps.api.addr_validate(&address)?;

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Desc) => (
            None,
            calc_range_end(start_after).map(Bound::exclusive),
            Order::Descending,
        ),
        _ => (
            calc_range_start(start_after).map(Bound::exclusive),
            None,
            Order::Ascending,
        ),
    };

    let current_time = env.block.time.seconds();
    let mut claimable_amount = Uint128::zero();
    let orders: Vec<(u64, Uint128)> = WITHDRAW_ORDERS
        .prefix(address.as_bytes())
        .range(deps.storage, start, end, order_by)
        .take(limit)
        .map(|item| {
            let (key, (_, prism_amt)) = item?;
            let end_time = u64::from_be_bytes(key.try_into().unwrap());
            if current_time > end_time {
                claimable_amount += prism_amt;
            }

            Ok((end_time, prism_amt))
        })
        .collect::<StdResult<Vec<(u64, Uint128)>>>()?;

    Ok(PrismWithdrawOrdersResponse {
        claimable_amount,
        orders,
    })
}
