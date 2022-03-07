use cosmwasm_std::{to_binary, Addr, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmQuery};
use prism_protocol::xprism_boost::{QueryMsg as BoostQueryMsg, UserInfo};

pub fn query_boost_amount(
    querier: &QuerierWrapper,
    boost_contract: &Addr,
    address: &Addr,
) -> StdResult<Uint128> {
    let res: UserInfo = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(boost_contract),
        msg: to_binary(&BoostQueryMsg::GetBoost {
            user: address.clone(),
        })?,
    }))?;

    Ok(res.total_boost)
}
