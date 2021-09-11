use cosmwasm_std::{Uint128, Decimal};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const DISTRIBUTION_STATUS: Item<DistributionStatus> = Item::new("distribution_status");
pub const BOND_AMOUNTS: Map<&[u8], Uint128> = Map::new("bond_amounts");

pub const REWARD_INFO: Map<&[u8], RewardInfo> = Map::new("reward_info");


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionStatus {
    pub total_distributed: Uint128,
    pub total_bond_amount: Uint128,
    pub pending_reward: Uint128,
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: String,
    pub prism_token: String,
    pub yluna_staking: String,
    pub yluna_token: String,
    pub distribution_schedule: (u64, u64, Uint128)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    pub pending_reward: Uint128, // not distributed amount due to zero bonding
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub pending_reward: Uint128,
}