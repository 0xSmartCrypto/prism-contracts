use cosmwasm_std::{to_binary, Addr, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmQuery};
use prism_protocol::vault::{QueryMsg as VaultQueryMsg, StateResponse};

pub fn query_vault_bond_amount(querier: &QuerierWrapper, vault: Addr) -> StdResult<Uint128> {
    let res: StateResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(vault),
        msg: to_binary(&VaultQueryMsg::State {})?,
    }))?;

    Ok(res.total_bond_amount)
}
