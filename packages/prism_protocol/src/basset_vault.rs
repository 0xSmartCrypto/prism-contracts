use crate::vault::BondedAmountResponse as VaultBondedAmountResponse;
use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type UnbondRequest = Vec<(u64, Uint128)>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub asset_name: String,
    pub asset_contract: String,
    pub asset_reward_contract: String,
    pub asset_reward_denom: String,
    pub token_admin: String,
    pub token_code_id: u64,
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
        reward_distribution: Option<String>,
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
    pub asset_name: String,
    pub asset_contract: String,
    pub asset_reward_contract: String,
    pub asset_reward_denom: String,
    pub casset_contract: String,
    pub passet_contract: String,
    pub yasset_contract: String,
    pub reward_distribution: String,
    pub initialized: bool,
    pub token_admin: String,
    pub token_code_id: u64,
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
