use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use terraswap::asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub cluna_token: String,
    pub yluna_token: String,
    pub pluna_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ////////////////////////
    /// User operations ///
    ////////////////////////
    /// Unbond yLUNA
    Unbond {
        amount: Uint128,
    },
    /// Withdraw pending rewards
    Withdraw {},

    /// Private methods
    /// 1) Swap delegator rewards to luna
    /// 2) LunaToCluna
    /// 3) ConvertAndDepositCluna
    ProcessDelegatorRewards {},

    LunaToCluna {},
    ConvertAndDepositCluna {},

    /// Deposit rewards to yLuna stakers
    DepositRewards {
        assets: Vec<Asset>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Bond yLuna to start receiving luna staking rewards
    Bond { mode: Option<String> },
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo { asset_token: String },
    RewardInfo { staker_addr: String },
    Whitelist {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoResponse {
    pub asset_token: String,
    pub reward_index: Decimal,
    pub pending_reward: Uint128,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub staker_addr: String,
    pub staked_amt: Uint128,
    pub staker_mode: Option<String>,
    pub reward_infos: Vec<Asset>,
}
