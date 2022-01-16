#[cfg(not(feature = "library"))]
use cosmwasm_std::{attr, to_binary, CosmosMsg, DepsMut, MessageInfo, Response, Uint128, WasmMsg};

use cw20::Cw20ExecuteMsg;

use crate::error::ContractResult;
use crate::state::LP_INFO;

pub fn split(deps: DepsMut, info: MessageInfo, amount: Uint128) -> ContractResult<Response> {
    let lp_info = LP_INFO.load(deps.storage)?;

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

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "split"),
        attr("from", info.sender.as_str()),
        attr("LP", lp_info.lp_contract.as_str()),
        attr("amount", amount),
    ]))
}

pub fn merge(deps: DepsMut, info: MessageInfo, amount: Uint128) -> ContractResult<Response> {
    let lp_info = LP_INFO.load(deps.storage)?;

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

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "merge"),
        attr("from", info.sender.as_str()),
        attr("LP", lp_info.lp_contract.as_str()),
        attr("amount", amount),
    ]))
}
