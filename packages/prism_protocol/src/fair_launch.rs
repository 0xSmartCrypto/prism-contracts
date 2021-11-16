use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LaunchConfig {
    pub amount: Uint128,
    // can deposit and withdraw
    pub phase1_start: u64,
    // can only withdraw
    pub phase2_start: u64,
    // can withdraw tokens
    pub phase2_end: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub token: String,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw { amount: Option<Uint128> },
    WithdrawTokens {},
    PostInitialize { launch_config: LaunchConfig },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    DepositInfo { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DepositResponse {
    pub address_deposit: Uint128,
    pub total_deposit: Uint128,
}
