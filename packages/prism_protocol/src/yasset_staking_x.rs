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

    /// PostInitialize to set the reward distribution contract
    PostInitialize {
        reward_distribution_contract: String,
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
    pub reward_distribution_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_bond_amount: Uint128,
    pub xyasset_supply: Uint128,
    pub exchange_rate: Decimal,
}
