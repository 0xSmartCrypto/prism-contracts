use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Coin, ContractResult, Decimal, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use cw20::BalanceResponse as Cw20BalanceResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use terra_cosmwasm::{
    ExchangeRateItem, ExchangeRatesResponse, TaxCapResponse, TaxRateResponse, TerraQuery,
    TerraQueryWrapper, TerraRoute,
};

pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let contract_addr = String::from(MOCK_CONTRACT_ADDR);
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(&contract_addr, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

#[derive(Clone, Default)]
pub struct BalanceQuerier {
    balances: HashMap<String, HashMap<String, Uint128>>,
}

impl BalanceQuerier {
    pub fn new(balances: &[(&String, &[(&String, &Uint128)])]) -> Self {
        BalanceQuerier {
            balances: balances_to_map(balances),
        }
    }
}

pub(crate) fn balances_to_map(
    balances: &[(&String, &[(&String, &Uint128)])],
) -> HashMap<String, HashMap<String, Uint128>> {
    let mut balances_map: HashMap<String, HashMap<String, Uint128>> = HashMap::new();
    for (contract_addr, balances) in balances.iter() {
        let mut contract_balances_map: HashMap<String, Uint128> = HashMap::new();
        for (asset, balance) in balances.iter() {
            contract_balances_map.insert(asset.to_string(), **balance);
        }

        balances_map.insert(contract_addr.to_string(), contract_balances_map);
    }
    balances_map
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    balance_querier: BalanceQuerier,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MockQueryMsg {
    Balance { address: String },
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if &TerraRoute::Treasury == route {
                    match query_data {
                        TerraQuery::TaxRate {} => {
                            let res = TaxRateResponse {
                                rate: Decimal::percent(1),
                            };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        TerraQuery::TaxCap { denom: _ } => {
                            let cap = Uint128::new(1000000u128);
                            let res = TaxCapResponse { cap };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else if &TerraRoute::Oracle == route {
                    match query_data {
                        TerraQuery::ExchangeRates {
                            base_denom,
                            quote_denoms,
                        } => {
                            let exchange_rates = quote_denoms
                                .iter()
                                .filter(|d| d != &"mnt")
                                .map(|e| ExchangeRateItem {
                                    quote_denom: e.clone(),
                                    exchange_rate: Decimal::from_str("22.1").unwrap(),
                                })
                                .collect();

                            SystemResult::Ok(ContractResult::from(to_binary(
                                &ExchangeRatesResponse {
                                    base_denom: base_denom.to_string(),
                                    exchange_rates,
                                },
                            )))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match from_binary(msg).unwrap() {
                    MockQueryMsg::Balance { address } => {
                        let balances: &HashMap<String, Uint128> =
                            match self.balance_querier.balances.get(contract_addr) {
                                Some(balances) => balances,
                                None => {
                                    return SystemResult::Err(SystemError::InvalidRequest {
                                        error: format!(
                                            "No balance info exists for the contract {}",
                                            contract_addr
                                        ),
                                        request: msg.as_slice().into(),
                                    })
                                }
                            };
                        let balance = match balances.get(&address) {
                            Some(v) => v,
                            None => {
                                return SystemResult::Err(SystemError::InvalidRequest {
                                    error: "Balance not found".to_string(),
                                    request: msg.as_slice().into(),
                                })
                            }
                        };

                        SystemResult::Ok(ContractResult::Ok(
                            to_binary(&Cw20BalanceResponse { balance: *balance }).unwrap(),
                        ))
                    }
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            balance_querier: BalanceQuerier::default(),
        }
    }

    pub fn with_balances(&mut self, balances: &[(&String, &[(&String, &Uint128)])]) {
        self.balance_querier = BalanceQuerier::new(balances);
    }
}
