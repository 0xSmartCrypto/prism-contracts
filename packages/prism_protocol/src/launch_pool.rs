use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::Asset;
use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub prism_token: String,
    pub reward_distribution: String,
    pub yasset_staking: String,
    pub yasset_token: String,
    // start, end, amount of $PRISM to distribute
    // distribute linearly
    pub distribution_schedule: (u64, u64, Uint128),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ////////////////////////
    /// User operations ///
    ////////////////////////
    /// Unbond yasset
    Unbond {
        amount: Option<Uint128>,
    },
    /// Withdraw $PRISM rewards
    /// Starts 21 day vesting period
    WithdrawRewards {},

    ClaimWithdrawnRewards {},

    /// Withdraw underlying rewards from yasset staking contract
    AdminWithdrawRewards {},

    /// Helper for AdminWithdrawRewards
    AdminSendWithdrawnRewards {
        original_balances: Vec<Asset>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Bond yasset to start receiving $PRISM rewards
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    DistributionStatus {},
    RewardInfo { staker_addr: String },
    VestingStatus { staker_addr: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub prism_token: String,
    pub reward_distribution: String,
    pub yasset_staking: String,
    pub yasset_token: String,
    pub distribution_schedule: (u64, u64, Uint128),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionStatusResponse {
    pub total_distributed: Uint128,
    pub total_bond_amount: Uint128,
    pub pending_reward: Uint128,
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub index: Decimal,
    pub pending_reward: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct VestingStatusResponse {
    pub scheduled_vests: Vec<(u64, Uint128)>,
    pub withdrawable: Uint128,
}
