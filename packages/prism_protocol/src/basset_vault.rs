use cosmwasm_std::{Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::vault::{BondedAmountResponse as VaultBondedAmountResponse};

pub type UnbondRequest = Vec<(u64, Uint128)>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub asset_contract: String,
    pub asset_reward_contract: String,
    pub asset_reward_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub total_bond_amount: Uint128,
    pub last_index_modification: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub creator: String,
    pub asset_contract: String, // todo: change to basset_contract?
    pub asset_reward_contract: String, 
    pub asset_reward_denom: String,
    pub casset_contract: Option<String>,
    pub yasset_contract: Option<String>,
    pub passet_contract: Option<String>,
    pub reward_distribution_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ////////////////////
    /// Owner's operations
    ////////////////////

    /// Set the owener
    UpdateConfig {
        owner: Option<String>,
        casset_contract: Option<String>,
        yasset_contract: Option<String>,
        passet_contract: Option<String>,
        reward_distribution_contract: Option<String>,
    },

    ////////////////////
    /// User's operations
    ////////////////////

    /// Receive interface Bond, BondSplit, and Unbond messages.
    Receive(Cw20ReceiveMsg),

    /// Split cAsset into yAsset and pAsset
    Split { amount: Uint128 },

    /// Merge yAsset and pAsset into cAsset
    Merge { amount: Uint128 },

    /// Update global index
    UpdateGlobalIndex {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    BondedAmount {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Bond {},
    BondSplit {},
    Unbond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub asset_contract: String,
    pub asset_reward_contract: String,
    pub asset_reward_denom: String,
    pub casset_contract: Option<String>,
    pub passet_contract: Option<String>,
    pub yasset_contract: Option<String>,
    pub reward_distribution_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_bond_amount: Uint128,
    pub last_index_modification: u64,
}

// we want same interface as vault for querying the bonded amount, necessary
// because this is queried by prism-reward-distribution contract, and it 
// needs to work with either vault type
pub type BondedAmountResponse = VaultBondedAmountResponse;
