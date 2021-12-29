use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env, Decimal, StdResult, Uint128, StdError, QueryRequest, QuerierWrapper, WasmQuery, to_binary,};

use crate::error::ContractError;
use crate::state::{CONFIG,};

use prism_protocol::lp_vault::{Config, ConfigResponse};

use cw20::{Cw20QueryMsg, TokenInfoResponse,};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(config.as_res()?)
}

pub fn query_token_info(querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<TokenInfoResponse> {
    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(contract_addr),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?)
}