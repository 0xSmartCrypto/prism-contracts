use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Uint128};
use prismswap::asset::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub base_denom: String,
    pub prism_token: String,
    pub fee_collector_addr: String,
    pub prism_ust_pair: String,
    pub order_fee: Decimal,
    pub min_fee_value: Uint128,
    pub executor_fee_portion: Decimal,
    pub excess_collector_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Owner operation to register a new pair
    AddPair {
        asset_infos: [AssetInfo; 2],
        pair_addr: String,
    },

    // Owner operation to update configuration
    UpdateConfig {
        owner: Option<String>,
        fee_collector_addr: Option<String>,
        order_fee: Option<Decimal>,
        min_fee_value: Option<Uint128>,
        executor_fee_portion: Option<Decimal>,
    },

    /// User submits a new order
    /// Before, the user should increase allowance for the offer_asset (or send the native token) and the fee
    SubmitOrder {
        offer_asset: Asset,
        ask_asset: Asset,
    },
    /// User operation to canel an existing order
    CancelOrder { order_id: u64 },
    /// Executor operation to execute an existing order
    ExecuteOrder { order_id: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Order {
        order_id: u64,
    },
    Orders {
        bidder_addr: Option<String>,
        start_after: Option<u64>,
        limit: Option<u32>,
        order_by: Option<OrderBy>,
    },
    LastOrderId {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub base_denom: String,
    pub prism_token: String,
    pub fee_collector_addr: String,
    pub prism_ust_pair: String,
    pub order_fee: Decimal,
    pub min_fee_value: Uint128,
    pub executor_fee_portion: Decimal,
    pub excess_collector_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OrderResponse {
    pub order_id: u64,
    pub bidder_addr: String,
    pub pair_addr: String,
    pub inter_pair_addr: Option<String>,
    pub offer_asset: Asset,
    pub ask_asset: Asset,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OrdersResponse {
    pub orders: Vec<OrderResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LastOrderIdResponse {
    pub last_order_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderBy {
    Asc,
    Desc,
}
