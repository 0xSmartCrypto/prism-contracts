#![allow(dead_code)]
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Coin, ContractResult, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult, WasmQuery,
};
use std::collections::HashMap;

use astroport::asset::AssetInfo;
use astroport::pair::{QueryMsg as PairQueryMsg, ReverseSimulationResponse, SimulationResponse};
use terra_cosmwasm::TerraQueryWrapper;

pub const MOCK_CONTRACT_ADDR: &str = "cosmos2contract";

pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let contract_addr = MOCK_CONTRACT_ADDR;
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(contract_addr, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    astroport_sim_querier: AstroportSimQuerier,
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

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match from_binary(msg).unwrap() {
                    PairQueryMsg::Simulation { offer_asset } => {
                        let res = self
                            .astroport_sim_querier
                            .sim_responses
                            .get(contract_addr)
                            .unwrap()
                            .get(&offer_asset.info.to_string())
                            .unwrap();
                        SystemResult::Ok(ContractResult::Ok(to_binary(&res).unwrap()))
                    }
                    PairQueryMsg::ReverseSimulation { ask_asset } => {
                        let res = self
                            .astroport_sim_querier
                            .reverse_sim_responses
                            .get(contract_addr)
                            .unwrap()
                            .get(&ask_asset.info.to_string())
                            .unwrap();
                        SystemResult::Ok(ContractResult::Ok(to_binary(&res).unwrap()))
                    }

                    _ => panic!("DO NOT ENTER HERE"),
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

#[derive(Clone, Default)]
pub struct AstroportSimQuerier {
    // pair_addr -> AssetInfo (str) -> SimulationResponse
    sim_responses: HashMap<String, HashMap<String, SimulationResponse>>,
    // pair_addr -> AssetInfo (str) -> ReverseSimulationResponse
    reverse_sim_responses: HashMap<String, HashMap<String, ReverseSimulationResponse>>,
}

impl AstroportSimQuerier {
    fn update_sim_response(
        &mut self,
        pair_addr: &str,
        offer_asset: &AssetInfo,
        sim_response: SimulationResponse,
    ) {
        self.sim_responses
            .entry(pair_addr.to_string())
            .or_insert(HashMap::new())
            .insert(offer_asset.to_string(), sim_response);
    }

    fn update_reverse_sim_response(
        &mut self,
        pair_addr: &str,
        offer_asset: &AssetInfo,
        reverse_sim_response: ReverseSimulationResponse,
    ) {
        self.reverse_sim_responses
            .entry(pair_addr.to_string())
            .or_insert(HashMap::new())
            .insert(offer_asset.to_string(), reverse_sim_response);
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            astroport_sim_querier: AstroportSimQuerier::default(),
        }
    }

    pub fn with_astroport_sim_response(
        &mut self,
        pair_addr: &str,
        offer_asset: &AssetInfo,
        sim_response: SimulationResponse,
    ) {
        self.astroport_sim_querier
            .update_sim_response(pair_addr, offer_asset, sim_response)
    }

    pub fn with_astroport_reverse_sim_response(
        &mut self,
        pair_addr: &str,
        ask_asset: &AssetInfo,
        reverse_sim_response: ReverseSimulationResponse,
    ) {
        self.astroport_sim_querier.update_reverse_sim_response(
            pair_addr,
            ask_asset,
            reverse_sim_response,
        )
    }
}
