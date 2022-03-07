use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use prism_protocol::launch_pool::{ConfigResponse, DistributionInfo, RewardInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const BASE_DISTRIBUTION_STATUS: Item<DistributionStatus> =
    Item::new("base_distribution_status");
pub const BOOST_DISTRIBUTION_STATUS: Item<DistributionStatus> =
    Item::new("boost_distribution_status");

pub const BOND_AMOUNTS: Map<&[u8], Uint128> = Map::new("bond_amounts");
pub const REWARD_INFO: Map<&[u8], RewardInfo> = Map::new("reward_info");

pub const SCHEDULED_VEST: Map<(&[u8], &[u8]), Uint128> = Map::new("scheduled_vest");
pub const PENDING_WITHDRAW: Map<&[u8], Uint128> = Map::new("pending_withdraw");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct DistributionStatus {
    pub total_distributed: Uint128,
    pub total_weight: Uint128,
    pub pending_reward: Uint128,
    pub reward_index: Decimal,
}

impl DistributionStatus {
    pub fn as_res(&self) -> DistributionInfo {
        DistributionInfo {
            total_distributed: self.total_distributed,
            total_weight: self.total_weight,
            pending_reward: self.pending_reward,
            reward_index: self.reward_index,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // Authorized to execute AdminWithdrawRewards and UpdateConfig. Usually a
    // human.
    pub owner: Addr,
    // Authorized to execute WithdrawRewardsBulk. Usually a bot.
    pub operator: Addr,
    pub prism_token: Addr,
    pub xprism_token: Addr,
    pub gov: Addr,
    pub yluna_staking: Addr,
    pub yluna_token: Addr,
    // How long rewards take to vest, in seconds.
    pub vesting_period: u64,
    pub boost_contract: Addr,
    pub distribution_schedule: (u64, u64, Uint128),
    pub base_pool_ratio: Decimal,
    /// An attempt to bond less than this amount will return an error. Useful
    /// to disallow trolls from creating 1 million addresses with 1 µ-yLuna each
    /// just to make `withdraw_rewards_bulk` more expensive.
    pub min_bond_amount: Uint128,
}

impl Config {
    pub fn as_res(&self) -> ConfigResponse {
        ConfigResponse {
            owner: self.owner.to_string(),
            operator: self.operator.to_string(),
            prism_token: self.prism_token.to_string(),
            xprism_token: self.xprism_token.to_string(),
            gov: self.gov.to_string(),
            yluna_staking: self.yluna_staking.to_string(),
            yluna_token: self.yluna_token.to_string(),
            vesting_period: self.vesting_period,
            boost_contract: self.boost_contract.to_string(),
            distribution_schedule: self.distribution_schedule,
            base_pool_ratio: self.base_pool_ratio,
            min_bond_amount: self.min_bond_amount,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub base_index: Decimal,
    pub boost_index: Decimal,
    pub active_boost: Uint128,
    pub boost_weight: Uint128,
    pub pending_reward: Uint128,
}

impl RewardInfo {
    pub fn as_res(&self, bond_amount: Uint128) -> RewardInfoResponse {
        RewardInfoResponse {
            bond_amount,
            base_index: self.base_index,
            boost_index: self.boost_index,
            boost_weight: self.boost_weight,
            pending_reward: self.pending_reward,
            active_boost: self.active_boost,
        }
    }
}
