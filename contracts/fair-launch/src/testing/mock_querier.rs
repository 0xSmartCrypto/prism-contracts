use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    to_binary, Binary, Coin, ContractResult, Decimal, OwnedDeps, SystemResult, Uint128,
};
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper};

const TAX_RATE: u64 = 5;
const TAX_CAP: u128 = 10;

/// A drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, MockQuerier<TerraQueryWrapper>> {
    let custom_querier: MockQuerier<TerraQueryWrapper> =
        MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)])
            .with_custom_handler(|query| SystemResult::Ok(custom_query_execute(&query)));
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub fn custom_query_execute(query: &TerraQueryWrapper) -> ContractResult<Binary> {
    match query.query_data {
        TerraQuery::TaxRate {} => {
            let resp = TaxRateResponse {
                rate: Decimal::percent(TAX_RATE),
            };
            to_binary(&resp).into()
        }
        TerraQuery::TaxCap { .. } => {
            let resp = TaxCapResponse {
                cap: Uint128::from(TAX_CAP),
            };
            to_binary(&resp).into()
        }
        _ => ContractResult::Err("Unhandled query: ".to_string()),
    }
}
