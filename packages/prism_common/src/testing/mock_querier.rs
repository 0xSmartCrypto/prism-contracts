use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Addr, Coin, ContractResult, Decimal, FullDelegation,
    OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, Validator,
    WasmQuery,
};
use cw20::TokenInfoResponse;
use std::collections::HashMap;
use std::str::FromStr;

use astroport::asset::{Asset, AssetInfo, PairInfo};
use astroport::factory::PairType;
use astroport::pair::{ReverseSimulationResponse, SimulationResponse};
use cw20::BalanceResponse as Cw20BalanceResponse;
use prism_protocol::vault::{StateResponse as VaultStateResponse, 
    BondedAmountResponse as VaultBondedAmountResponse};
use prism_protocol::yasset_staking::{StateResponse as YassetStakingStateResponse}; 
use prism_protocol::yasset_staking_x::{StateResponse as YassetStakingXStateResponse}; 
use prism_protocol::reward_distribution::RewardAssetWhitelistResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terra_cosmwasm::{
    ExchangeRateItem, ExchangeRatesResponse, TaxCapResponse, TaxRateResponse, TerraQuery,
    TerraQueryWrapper, TerraRoute,
};

pub const MOCK_CONTRACT_ADDR: &str = "cosmos2contract";
pub const VAULT: &str = "vault";
pub const YASSET_STAKING: &str = "yasset_staking";
pub const YASSET_STAKING_X: &str = "yasset_staking_x";

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

#[derive(Clone, Default)]
pub struct TaxQuerier {
    rate: Decimal,
    caps: HashMap<String, Uint128>,
}

impl TaxQuerier {
    pub fn _new(rate: Decimal, caps: &[(&String, &Uint128)]) -> Self {
        TaxQuerier {
            rate,
            caps: _caps_to_map(caps),
        }
    }
}

pub(crate) fn _caps_to_map(caps: &[(&String, &Uint128)]) -> HashMap<String, Uint128> {
    let mut owner_map: HashMap<String, Uint128> = HashMap::new();
    for (denom, cap) in caps.iter() {
        owner_map.insert(denom.to_string(), **cap);
    }
    owner_map
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    token_querier: TokenQuerier,
    tax_querier: TaxQuerier,
    factory_querier: FactoryQuerier,
    vault_state_querier: VaultStateQuerier,
    astroport_sim_querier: AstroportSimQuerier,
    yasset_staking_state_querier: YassetStakingStateQuerier,
    yasset_staking_x_state_querier: YassetStakingXStateQuerier,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Pair { asset_infos: [AssetInfo; 2] },
    Balance { address: String },
    TokenInfo {},
    State {},
    RewardAssetWhitelist {},
    Simulation { offer_asset: Asset },
    ReverseSimulation { ask_asset: Asset },
    BondedAmount {},
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if &TerraRoute::Treasury == route {
                    match query_data {
                        TerraQuery::TaxRate {} => {
                            let res = TaxRateResponse {
                                rate: self.tax_querier.rate,
                            };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        TerraQuery::TaxCap { denom } => {
                            let cap = self
                                .tax_querier
                                .caps
                                .get(denom)
                                .copied()
                                .unwrap_or_default();
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
                    println!("request: {:?}", request);
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match from_binary(msg).unwrap() {
                    QueryMsg::Pair { asset_infos } => {
                        let key = asset_infos[0].to_string() + asset_infos[1].to_string().as_str();
                        match self.factory_querier.pairs.get(&key) {
                            Some(v) => {
                                SystemResult::Ok(ContractResult::from(to_binary(&PairInfo {
                                    pair_type: PairType::Xyk {},
                                    contract_addr: Addr::unchecked(v),
                                    liquidity_token: Addr::unchecked("liquidity".to_string()),
                                    asset_infos: [
                                        AssetInfo::NativeToken {
                                            denom: "uusd".to_string(),
                                        },
                                        AssetInfo::NativeToken {
                                            denom: "uusd".to_string(),
                                        },
                                    ],
                                })))
                            }
                            None => SystemResult::Err(SystemError::InvalidRequest {
                                error: "No pair info exists".to_string(),
                                request: msg.as_slice().into(),
                            }),
                        }
                    }
                    QueryMsg::TokenInfo {} => {
                        let mut total_supply = Uint128::zero();
                        if let Some(balances) = self.token_querier.balances.get(contract_addr) {
                            for balance in balances {
                                total_supply += *balance.1;
                            }
                        }
                        let token_inf: TokenInfoResponse = TokenInfoResponse {
                            name: "pLuna".to_string(),
                            symbol: "pLUNA".to_string(),
                            decimals: 6,
                            total_supply,
                        };
                        SystemResult::Ok(ContractResult::Ok(to_binary(&token_inf).unwrap()))
                    }
                    QueryMsg::Balance { address } => {
                        let balances: &HashMap<String, Uint128> =
                            match self.token_querier.balances.get(contract_addr) {
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
                            Some(v) => *v,
                            None => {
                                return SystemResult::Ok(ContractResult::Ok(
                                    to_binary(&Cw20BalanceResponse {
                                        balance: Uint128::zero(),
                                    })
                                    .unwrap(),
                                ));
                            }
                        };

                        SystemResult::Ok(ContractResult::Ok(
                            to_binary(&Cw20BalanceResponse { balance }).unwrap(),
                        ))
                    }
                    QueryMsg::State {} => {
                        match contract_addr.as_str() {
                            VAULT => {
                                SystemResult::Ok(ContractResult::Ok(
                                    to_binary(&VaultStateResponse {
                                        exchange_rate: Decimal::one(),
                                        total_bond_amount: self.vault_state_querier.total_bond_amount,
                                        last_index_modification: 0u64,
                                        prev_vault_balance: Uint128::zero(),
                                        actual_unbonded_amount: Uint128::zero(),
                                        last_unbonded_time: 0u64,
                                        last_processed_batch: 0u64,
                                    })
                                    .unwrap(),
                            ))}
                            YASSET_STAKING => {
                                SystemResult::Ok(ContractResult::Ok(
                                    to_binary(&YassetStakingStateResponse {
                                        total_bond_amount: self.yasset_staking_state_querier.total_bond_amount,
                                    })
                                    .unwrap(),
                            ))}
                            YASSET_STAKING_X => {
                                SystemResult::Ok(ContractResult::Ok(
                                    to_binary(&YassetStakingXStateResponse {
                                        total_bond_amount: self.yasset_staking_x_state_querier.total_bond_amount,
                                        xyasset_supply: Uint128::zero(),
                                        exchange_rate: Decimal::zero(),
                                    })
                                    .unwrap(),
                            ))}
                            _ => {
                                return SystemResult::Err(SystemError::InvalidRequest {
                                    error: format!(
                                        "No state info exists for the contract {}",
                                        contract_addr
                                    ),
                                    request: msg.as_slice().into(),
                                })
                            } 
                        }
                    }
                    QueryMsg::BondedAmount {} => {
                        SystemResult::Ok(ContractResult::Ok(
                        to_binary(&VaultBondedAmountResponse {
                            total_bond_amount: self.vault_state_querier.total_bond_amount,
                        })
                        .unwrap(),
                    ))}
                    QueryMsg::RewardAssetWhitelist {} => SystemResult::Ok(ContractResult::Ok(
                        to_binary(&RewardAssetWhitelistResponse {
                            assets: vec![
                                AssetInfo::NativeToken {
                                    denom: "uluna".to_string(),
                                },
                                AssetInfo::Token {
                                    contract_addr: Addr::unchecked("mir0000"),
                                },
                            ],
                        })
                        .unwrap(),
                    )),
                    QueryMsg::Simulation { offer_asset } => {
                        let res = self
                            .astroport_sim_querier
                            .sim_responses
                            .get(&(contract_addr.to_string(), offer_asset.info.to_string()))
                            .unwrap();
                        SystemResult::Ok(ContractResult::Ok(to_binary(&res).unwrap()))
                    }
                    QueryMsg::ReverseSimulation { ask_asset } => {
                        let res = self
                            .astroport_sim_querier
                            .reverse_sim_responses
                            .get(&(contract_addr.to_string(), ask_asset.info.to_string()))
                            .unwrap();
                        SystemResult::Ok(ContractResult::Ok(to_binary(&res).unwrap()))
                    }
                }
            }
            _ => self.base.handle_query(request),
        }
    }
    pub fn update_staking(
        &mut self,
        denom: &str,
        validators: &[Validator],
        delegations: &[FullDelegation],
    ) {
        self.base.update_staking(denom, validators, delegations);
    }
}

#[derive(Clone, Default)]
pub struct TokenQuerier {
    balances: HashMap<String, HashMap<String, Uint128>>,
}

impl TokenQuerier {
    pub fn new(balances: &[(&String, &[(&String, &Uint128)])]) -> Self {
        TokenQuerier {
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
        for (addr, balance) in balances.iter() {
            contract_balances_map.insert(addr.to_string(), **balance);
        }

        balances_map.insert(contract_addr.to_string(), contract_balances_map);
    }
    balances_map
}

#[derive(Clone, Default)]
pub struct FactoryQuerier {
    pairs: HashMap<String, String>,
}

impl FactoryQuerier {
    pub fn new(pairs: &[(&String, &String)]) -> Self {
        FactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &String)]) -> HashMap<String, String> {
    let mut pairs_map: HashMap<String, String> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(key.to_string(), pair.to_string());
    }
    pairs_map
}

#[derive(Clone, Default)]
pub struct VaultStateQuerier {
    total_bond_amount: Uint128,
}

impl VaultStateQuerier {
    pub fn new(total_bond_amount: &Uint128) -> Self {
        VaultStateQuerier {
            total_bond_amount: *total_bond_amount,
        }
    }
}

#[derive(Clone, Default)]
pub struct AstroportSimQuerier {
    // (pair_addr, asset) -> SimulationResponse
    sim_responses: HashMap<(String, String), SimulationResponse>,
    // (pair_addr, asset) -> ReverseSimulationResponse
    reverse_sim_responses: HashMap<(String, String), ReverseSimulationResponse>,
}

impl AstroportSimQuerier {
    fn update_sim_response(
        &mut self,
        pair_addr: &str,
        offer_asset: &AssetInfo,
        response: SimulationResponse,
    ) {
        self.sim_responses
            .insert((pair_addr.to_string(), offer_asset.to_string()), response);
    }

    fn update_reverse_sim_response(
        &mut self,
        pair_addr: &str,
        ask_asset: &AssetInfo,
        response: ReverseSimulationResponse,
    ) {
        self.reverse_sim_responses
            .insert((pair_addr.to_string(), ask_asset.to_string()), response);
    }
}

#[derive(Clone, Default)]
pub struct YassetStakingStateQuerier {
    total_bond_amount: Uint128,
}

impl YassetStakingStateQuerier {
    pub fn new(total_bond_amount: &Uint128) -> Self {
        YassetStakingStateQuerier {
            total_bond_amount: *total_bond_amount,
        }
    }
}

#[derive(Clone, Default)]
pub struct YassetStakingXStateQuerier {
    total_bond_amount: Uint128,
}

impl YassetStakingXStateQuerier {
    pub fn new(total_bond_amount: &Uint128) -> Self {
        YassetStakingXStateQuerier {
            total_bond_amount: *total_bond_amount,
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            token_querier: TokenQuerier::default(),
            tax_querier: TaxQuerier::default(),
            factory_querier: FactoryQuerier::default(),
            vault_state_querier: VaultStateQuerier::default(),
            astroport_sim_querier: AstroportSimQuerier::default(),
            yasset_staking_state_querier: YassetStakingStateQuerier::default(),
            yasset_staking_x_state_querier: YassetStakingXStateQuerier::default(),
        }
    }

    pub fn with_native_balances(&mut self, balances: &[(String, Coin)]) {
        for (addr, coin) in balances {
            self.base.update_balance(addr, vec![coin.clone()]);
        }
    }

    pub fn with_token_balances(&mut self, balances: &[(&String, &[(&String, &Uint128)])]) {
        self.token_querier = TokenQuerier::new(balances);
    }

    pub fn with_tax(&mut self, rate: Decimal, caps: &[(&String, &Uint128)]) {
        self.tax_querier = TaxQuerier::_new(rate, caps);
    }

    pub fn with_pairs(&mut self, pairs: &[(&String, &String)]) {
        self.factory_querier = FactoryQuerier::new(pairs);
    }

    pub fn with_vault_state(&mut self, total_bond_amount: &Uint128) {
        self.vault_state_querier = VaultStateQuerier::new(total_bond_amount);
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
    
    pub fn with_yasset_staking_state(&mut self, total_bond_amount: &Uint128) {
        self.yasset_staking_state_querier = YassetStakingStateQuerier::new(total_bond_amount);
    }

    pub fn with_yasset_staking_x_state(&mut self, total_bond_amount: &Uint128) {
        self.yasset_staking_x_state_querier = YassetStakingXStateQuerier::new(total_bond_amount);
    }

}
