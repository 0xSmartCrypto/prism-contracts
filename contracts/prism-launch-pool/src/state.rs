use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use prism_protocol::launch_pool::{ConfigResponse, DistributionStatusResponse, RewardInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");

/// DISTRIBUTION_STATUS is a global object (not tied to an individual user) that
/// summarizes information used to compute rewards.
pub const DISTRIBUTION_STATUS: Item<DistributionStatus> = Item::new("distribution_status");

/// BOND_AMOUNTS is map that tells how much each user has bound.
///
/// Key: user address, Value: number of ylunas that this user has bound.
///
/// When Bond is called, the user's entry is incremented and upserted. When
/// Unbond is called, the user's entry is decremented (but never removed from
/// the map).
pub const BOND_AMOUNTS: Map<&[u8], Uint128> = Map::new("bond_amounts");

pub const REWARD_INFO: Map<&[u8], RewardInfo> = Map::new("reward_info");

/// SCHEDULED_VEST holds amounts of PRISM that should be released to users in
/// the future.
///
/// Key: Pair of:
///  - user address (Addr)
///  - timestamp when funds are released in seconds (u64)
///
/// Value: Amount of PRISM that will be released.
pub const SCHEDULED_VEST: Map<(&[u8], &[u8]), Uint128> = Map::new("scheduled_vest");

/// PENDING_WITHDRAW indicates how much PRISM has already vested per user.
///
/// Key: user address, Value: amount of PRISM ready to be transfered to the user
/// right now.
pub const PENDING_WITHDRAW: Map<&[u8], Uint128> = Map::new("pending_withdraw");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct DistributionStatus {
    pub total_distributed: Uint128,
    /// total_bond_amount is the total amount of yluna that has been bonded by
    /// users. It starts at 0 when this contract is instantiated. It gets
    /// incremented when Bond is called and decremented when Unbond is called.
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
