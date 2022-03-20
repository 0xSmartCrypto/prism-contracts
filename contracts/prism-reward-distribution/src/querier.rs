use cosmwasm_std::{to_binary, Addr, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmQuery};
use prism_protocol::vault::{
    BondedAmountResponse as VaultBondedAmountResponse, QueryMsg as VaultQueryMsg,
};
use prism_protocol::yasset_staking::{
    QueryMsg as YassetStakingQueryMsg, StateResponse as YassetStakingStateResponse,
};
use prism_protocol::yasset_staking_x::{
    QueryMsg as YassetStakingXQueryMsg, StateResponse as YassetStakingXStateResponse,
};

pub fn query_vault_bond_amount(querier: &QuerierWrapper, vault: Addr) -> StdResult<Uint128> {
    let res: VaultBondedAmountResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(vault),
        msg: to_binary(&VaultQueryMsg::BondedAmount {})?,
    }))?;

    Ok(res.total_bond_amount)
}

pub fn query_yasset_staking_bond_amount(
    querier: &QuerierWrapper,
    yasset_staking: Addr,
) -> StdResult<Uint128> {
    let res: YassetStakingStateResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(yasset_staking),
        msg: to_binary(&YassetStakingQueryMsg::State {})?,
    }))?;

    Ok(res.total_bond_amount)
}

pub fn query_yasset_staking_x_bond_amount(
    querier: &QuerierWrapper,
    yasset_staking_x: Addr,
) -> StdResult<Uint128> {
    let res: YassetStakingXStateResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: String::from(yasset_staking_x),
            msg: to_binary(&YassetStakingXQueryMsg::State {})?,
        }))?;
    Ok(res.total_bond_amount)
}
