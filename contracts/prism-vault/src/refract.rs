use crate::state::CONFIG;
use cosmwasm_std::{
    to_binary, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg as TokenMsg;

pub fn split(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint128) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?.assert_initialized()?;
    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.cluna_contract.to_string(),
            msg: to_binary(&TokenMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.yluna_contract.to_string(),
            msg: to_binary(&TokenMsg::Mint {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.pluna_contract.to_string(),
            msg: to_binary(&TokenMsg::Mint {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
    ];
    Ok(Response::new().add_messages(messages))
}

pub fn merge(deps: DepsMut, info: MessageInfo, amount: Uint128) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?.assert_initialized()?;
    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.yluna_contract.to_string(),
            msg: to_binary(&TokenMsg::BurnFrom {
                owner: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.pluna_contract.to_string(),
            msg: to_binary(&TokenMsg::BurnFrom {
                owner: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.cluna_contract.to_string(),
            msg: to_binary(&TokenMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
    ];
    Ok(Response::new().add_messages(messages))
}
