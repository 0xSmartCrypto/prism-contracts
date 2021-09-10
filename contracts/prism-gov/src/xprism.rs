use crate::state::config_read;
use cosmwasm_std::{to_binary, CosmosMsg, DepsMut, Env, Response, StdResult, Uint128, WasmMsg};
use cw20_base::msg::ExecuteMsg as TokenMsg;
use terraswap::querier::query_token_balance;

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

pub fn redeem_xprism(
    deps: DepsMut,
    env: Env,
    sender: String,
    amount: Uint128,
) -> StdResult<Response> {
    let cfg = config_read(deps.storage).load()?;
    let prism_token = deps.api.addr_humanize(&cfg.prism_token)?;
    let xprism_token = deps.api.addr_humanize(&cfg.xprism_token)?;

    let prism_amt = query_token_balance(
        &deps.querier,
        prism_token.clone(),
        env.contract.address.clone(),
    )?;
    let xprism_amt =
        query_token_balance(&deps.querier, xprism_token.clone(), env.contract.address)?;

    let prism_to_return = amount.multiply_ratio(prism_amt, xprism_amt);

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: xprism_token.clone().into_string(),
            msg: to_binary(&TokenMsg::Burn { amount })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: prism_token.clone().into_string(),
            msg: to_binary(&TokenMsg::Transfer {
                recipient: sender,
                amount: prism_to_return,
            })?,
            funds: vec![],
        }),
    ]))
}
