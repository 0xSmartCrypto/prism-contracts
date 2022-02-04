use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub prism_token: String,
    /// vector of (start time, end time, reward amount)
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
    /// vector of (staking token, weight, unbond period)
    pub staking_tokens: Vec<(String, u64, u64)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    UpdateOwner {
        owner: String,
    },
    AddDistributionSchedule {
        schedule: Vec<(u64, u64, Uint128)>,
    },
    RegisterStakingToken {
        staking_token: String,
        unbond_period: u64,
        weight: u64,
    },
    UpdateStakingToken {
        staking_token: String,
        unbond_period: Option<u64>,
        weight: Option<u64>,
    },
    Unbond {
        staking_token: String,
        amount: Option<Uint128>,
    },
    ClaimUnbonded {
        staking_token: String,
    },
    ClaimRewards {
        staking_token: Option<String>,
    },
    AutoStakeHook {
        staking_token: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo {
        staking_token: String,
    },
    StakerInfo {
        staker: String,
        staking_token: Option<String>,
    },
    TokenStakersInfo {
        staking_token: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    UnbondOrders {
        staking_token: String,
        staker: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub prism_token: String,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
    pub total_weight: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoResponse {
    pub weight: u64,
    pub last_distributed: u64,
    pub staking_token: String,
    pub total_bond_amount: Uint128,
    pub reward_index: Decimal,
    pub pending_reward: Uint128,
    pub unbond_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfoResponse {
    pub staker: String,
    pub reward_infos: Vec<RewardInfoResponseItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakersInfoResponse {
    pub stakers: Vec<StakerInfoResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponseItem {
    pub staking_token: String,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
    pub withdrawable_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondOrdersResponse {
    pub withdrawable_amount: Uint128,
    /// vector of (time available for withdrawal, amount)
    pub orders: Vec<(u64, Uint128)>,
}
