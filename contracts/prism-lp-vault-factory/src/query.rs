use cosmwasm_std::{
    Addr, Deps, StdResult, QuerierWrapper, QueryRequest, WasmQuery, to_binary
};

use cw20::{MinterResponse, TokenInfoResponse, Cw20QueryMsg};

use prism_protocol::lp_vault_factory::{Config, LPContracts};
use crate::state::{CONFIG, VAULTS};

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn query_vault(deps: Deps, amm: u64, lp: &Addr) -> StdResult<LPContracts> {
    VAULTS.load(deps.storage, (amm.into(), lp))
}

pub fn query_token_info(
    querier: &QuerierWrapper,
    contract_addr: Addr,
) -> StdResult<TokenInfoResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.into_string(),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))
}
