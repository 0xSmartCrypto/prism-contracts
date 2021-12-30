use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env, Decimal, StdResult, Uint128, StdError, CosmosMsg, WasmMsg, QueryRequest, QuerierWrapper, WasmQuery, Response, to_binary,};

use crate::error::ContractError;
use crate::state::{CONFIG,};

use prism_protocol::lp_vault::{Config, ConfigResponse};
use astroport::asset::PairInfo;
use astroport::factory::{QueryMsg as AstroQueryMsg, ConfigResponse as FactoryConfigResponse};

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

pub fn query_all_pairs(deps: Deps, querier: &QuerierWrapper) -> StdResult<Vec<PairInfo>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // grab all pairs from astroport
    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.clone(),
        msg: to_binary(&AstroQueryMsg::Pairs { 
            limit: None,
            start_after: None,
        })?,
    }))?)
}

// pretty inefficient for now... may be some other way to go from liquidity token contract addr -> pair contract addr
pub fn query_pair_info(deps: Deps, querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<PairInfo> {
    let config: Config = CONFIG.load(deps.storage)?;
    
    // find PairInfo with equivalent LP token
    Ok(query_all_pairs(deps, &querier)?
        .into_iter()
        .find(|x| x.liquidity_token == contract_addr)
        .ok_or_else(|| {
            StdError::generic_err("LP Token not found in Astroport")
        })?
    )
}

pub fn query_factory_config(deps: Deps, querier: &QuerierWrapper) -> StdResult<FactoryConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    // query for factory config and return its token code id
    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.clone(),
        msg: to_binary(&AstroQueryMsg::Config {})?,
    }))?)
}