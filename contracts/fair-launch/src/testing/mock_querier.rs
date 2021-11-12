use cosmwasm_std::{
  to_binary, Binary, Coin, ContractResult, Decimal,
  OwnedDeps, SystemResult, Uint128,
};
use cosmwasm_std::testing::{MockApi, MockStorage, MockQuerier, MOCK_CONTRACT_ADDR};
use terra_cosmwasm::{TerraQuery, TerraQueryWrapper, TaxCapResponse, TaxRateResponse};


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
          let resp = TaxRateResponse { rate: Decimal::percent(5) };
          to_binary(&resp).into()
      }
      TerraQuery::TaxCap { .. } => {
          let resp = TaxCapResponse { cap: Uint128::from(10u128) };
          to_binary(&resp).into()
      }
      _ => ContractResult::Err("Unhandled query: ".to_string())
  }
}
