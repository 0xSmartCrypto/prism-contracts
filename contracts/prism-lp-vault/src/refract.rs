#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, DepsMut, MessageInfo, Response, 
    StdError, StdResult, Uint128, WasmMsg,
};

use cw20::Cw20ExecuteMsg;

use crate::state::{LP_IDS, LP_INFOS};

pub fn split(
    deps: DepsMut,
    info: MessageInfo,
    token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;
    let lp_info = LP_INFOS
        .load(deps.storage, lp_id.into())
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;

    // burn cLP, mint p/yLP
    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.clp_contract.clone().into_string(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.clone().into_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.plp_contract.clone().into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.clone().into_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.ylp_contract.into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.clone().into_string(),
                amount,
            })?,
            funds: vec![],
        }),
    ];

    Ok(Response::new().add_messages(messages)
                      .add_attributes(vec![
                          attr("action", "split"),
                          attr("from", info.sender.as_str()),
                          attr("LP", lp_info.lp_contract.as_str()),
                          attr("amount", amount),
                      ]))
}

pub fn merge(
    deps: DepsMut,
    info: MessageInfo,
    token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No LP address exists".to_string()))?;
    let lp_info = LP_INFOS
        .load(deps.storage, lp_id.into())
        .map_err(|_| StdError::generic_err("No LP address exists".to_string()))?;

    // burn p/yLP, mint cLP
    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.plp_contract.clone().into_string(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.clone().into_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.ylp_contract.clone().into_string(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.clone().into_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.clp_contract.into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.clone().into_string(),
                amount,
            })?,
            funds: vec![],
        }),
    ];

    Ok(Response::new().add_messages(messages)
                      .add_attributes(vec![
                          attr("action", "merge"),
                          attr("from", info.sender.as_str()),
                          attr("LP", lp_info.lp_contract.as_str()),
                          attr("amount", amount),
                      ]))
}
