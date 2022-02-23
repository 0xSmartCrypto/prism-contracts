use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use prism_protocol::launch_pool::{ConfigResponse, DistributionStatusResponse, RewardInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const DISTRIBUTION_STATUS: Item<DistributionStatus> = Item::new("distribution_status");
pub const BOND_AMOUNTS: Map<&[u8], Uint128> = Map::new("bond_amounts");

pub const REWARD_INFO: Map<&[u8], RewardInfo> = Map::new("reward_info");

pub const SCHEDULED_VEST: Map<(&[u8], &[u8]), Uint128> = Map::new("scheduled_vest");
pub const PENDING_WITHDRAW: Map<&[u8], Uint128> = Map::new("pending_withdraw");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct DistributionStatus {
    pub total_distributed: Uint128,
    pub total_bond_amount: Uint128,
    pub pending_reward: Uint128,
    pub reward_index: Decimal,
}

impl DistributionStatus {
    pub fn as_res(&self) -> DistributionStatusResponse {
        DistributionStatusResponse {
            total_distributed: self.total_distributed,
            total_bond_amount: self.total_bond_amount,
            pending_reward: self.pending_reward,
            reward_index: self.reward_index,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub prism_token: Addr,
    pub yluna_staking: Addr,
    pub yluna_token: Addr,
    pub distribution_schedule: (u64, u64, Uint128),
}

impl Config {
    pub fn as_res(&self) -> ConfigResponse {
        ConfigResponse {
            owner: self.owner.to_string(),
            prism_token: self.prism_token.to_string(),
            yluna_staking: self.yluna_staking.to_string(),
            yluna_token: self.yluna_token.to_string(),
            distribution_schedule: self.distribution_schedule,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub pending_reward: Uint128,
}

impl RewardInfo {
    pub fn as_res(&self) -> RewardInfoResponse {
        RewardInfoResponse {
            index: self.index,
            pending_reward: self.pending_reward,
        }
    }
}
