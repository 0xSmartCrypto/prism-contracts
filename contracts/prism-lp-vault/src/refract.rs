#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg, Reply, ReplyOn, Decimal,
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};
use astroport::token::{InstantiateMsg as AstroTokenInstantiateMsg};
use astroport::factory::{ConfigResponse as FactoryConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS, LPInfo};
use crate::query::{query_config, query_token_info, query_pair_info, query_factory_config};

use crate::response::MsgInstantiateContractResponse;
use protobuf::Message;

use astroport::asset::{AssetInfo, addr_validate_to_lower};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, TokenInfoResponse, MinterResponse};
use terra_cosmwasm::TerraMsgWrapper;

// these currently work with any LP address in the set
// should it be restricted to just cLP?

pub fn split(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    // make sure the LP token exists
    let lp_id = LP_IDS.load(deps.storage, &token.clone())
                      .map_err(|_| StdError::generic_err(format!("No cLP address exists")))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())
                              .map_err(|_| StdError::generic_err(format!("No cLP address exists")))?;

    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.plp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn merge(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Uint128,
) -> StdResult<Response> {
    let token_addr = deps.api.addr_validate(&token)?;
    let lp_id = LP_IDS.load(deps.storage, &token_addr)
                      .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())
                              .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;

    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.plp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}