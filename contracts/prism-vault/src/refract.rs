use crate::state::CONFIG;
use cosmwasm_std::{
    to_binary, CosmosMsg, DepsMut, MessageInfo, Response, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg as TokenMsg;

pub fn split(deps: DepsMut, info: MessageInfo, amount: Uint128) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.cluna_contract.unwrap(),
        msg: to_binary(&TokenMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.yluna_contract.unwrap(),
        msg: to_binary(&TokenMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.pluna_contract.unwrap(),
        msg: to_binary(&TokenMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn merge(deps: DepsMut, info: MessageInfo, amount: Uint128) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.yluna_contract.unwrap(),
        msg: to_binary(&TokenMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.pluna_contract.unwrap(),
        msg: to_binary(&TokenMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.cluna_contract.unwrap(),
        msg: to_binary(&TokenMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}
