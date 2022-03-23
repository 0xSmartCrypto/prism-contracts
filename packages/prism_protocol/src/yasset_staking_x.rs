use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub yasset_token: String,
    pub prism_token: String,
    pub prism_yasset_pair: String,
    pub collector: String,
    pub reward_distribution: String,
    pub token_code_id: u64, // cw20 token code id for xyasset token creation
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Bond and Unbond operations, received token must be yasset
    Receive(Cw20ReceiveMsg),

    /// Deposit rewards to stakers, this converts incoming assets to the
    /// underlying yasset (thereby appreciating the xyasset)
    DepositRewards {
        assets: Vec<Asset>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Bond {},
    Unbond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub yasset_token: String,
    pub xyasset_token: String,
    pub prism_token: String,
    pub collector: String,
    pub reward_distribution: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    /// total_bond_amount is same as xyasset supply
    pub total_bond_amount: Uint128,

    /// current balance of yasset token
    pub yasset_balance: Uint128,

    /// exchange rate is yasset_balance / total_bond_amount (ie xyaset supply)
    pub exchange_rate: Decimal,
}
