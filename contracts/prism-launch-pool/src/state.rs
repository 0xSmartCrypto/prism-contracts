use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use prism_protocol::launch_pool::{ConfigResponse, DistributionInfo, RewardInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");

/// BASE_DISTRIBUTION_STATUS is a global object (not tied to an individual user) that summarizes information used to compute
/// rewards.
pub const BASE_DISTRIBUTION_STATUS: Item<DistributionStatus> =
    Item::new("base_distribution_status");
pub const BOOST_DISTRIBUTION_STATUS: Item<DistributionStatus> =
    Item::new("boost_distribution_status");

/// BOND_AMOUNTS is map that tells how much each user has bound.
///
/// Key: user address, Value: number of ylunas that this user has bound.
///
/// When Bond is called, the user's entry is incremented and upserted. When Unbond is called, the user's entry is
/// decremented (but never removed from the map).
pub const BOND_AMOUNTS: Map<&[u8], Uint128> = Map::new("bond_amounts");

/// REWARD_INFO keeps track of rewards that have been earned by users (but haven't vested yet).
/// Key: user address.
pub const REWARD_INFO: Map<&[u8], RewardInfo> = Map::new("reward_info");

/// SCHEDULED_VEST holds amounts of PRISM that have already been earned by users but will vest in the future.
///
/// Key: Pair of:
///  - user address (Addr)
///  - timestamp when funds are released in seconds (u64)
///
/// Value: Amount of PRISM that will be released.
pub const SCHEDULED_VEST: Map<(&[u8], &[u8]), Uint128> = Map::new("scheduled_vest");

/// PENDING_WITHDRAW indicates how much PRISM has already vested per user.
///
/// Key: user address, Value: amount of PRISM ready to be transferred to the user right now.
pub const PENDING_WITHDRAW: Map<&[u8], Uint128> = Map::new("pending_withdraw");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct DistributionStatus {
    /// total_distributed is the cumulative amount of rewards that have been distributed by the protocol since the
    /// schedule started and up to the time when this field was stored. "Distributed" here just means those rewards were
    /// either added to pending_reward or added to reward_index (not actually transferred out of the contract to people
    /// yet). Units: PRISM tokens.
    pub total_distributed: Uint128,
    /// total_weight is the total amount of yluna that has been bonded by users. It starts at 0 when this contract
    /// is instantiated. It gets incremented when Bond is called and decremented when Unbond is called. Units: yluna
    /// tokens.
    pub total_weight: Uint128,
    /// pending_reward is used to count rewards that should have been given to people according to the schedule but
    /// weren't actually given to anybody because there were no bonders at the moment (i.e. total_bond_amount was 0).
    /// These rewards are saved for lucky future bonders. In practice this probably never happens because there's always
    /// at least one bonder. Units: PRISM tokens.
    pub pending_reward: Uint128,
    /// reward_index is part of a trick to lazily compute each user's actual earned rewards in an efficient manner.
    /// "Index" is a bit of a misnomer since it doesn't mean the typical "index" of an array. Perhaps a better name
    /// would be "cumulative_piecewise_rewards_per_bonded_unit".
    ///
    /// reward_index is a monotonically increasing value (i.e. it only grows, it never decreases). See documentation at
    /// [https://github.com/prism-finance/prism-contracts/blob/main/rewards_index_explanation.md] for more details.
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
    pub owner: Addr,
    pub operator: Addr,
    pub prism_token: Addr,
    pub yluna_staking: Addr,
    pub yluna_token: Addr,
    pub vesting_period: u64,
    pub boost_contract: Addr,
    /// base_distribution_schedule is a triple of:
    ///   - start timestamp in seconds;
    ///   - end timestamp in seconds;
    ///   - amount of tokens to be distributed as rewards during this time period.
    pub base_distribution_schedule: (u64, u64, Uint128),
    pub boost_distribution_schedule: (u64, u64, Uint128),
}

impl Config {
    pub fn as_res(&self) -> ConfigResponse {
        ConfigResponse {
            owner: self.owner.to_string(),
            operator: self.operator.to_string(),
            prism_token: self.prism_token.to_string(),
            yluna_staking: self.yluna_staking.to_string(),
            yluna_token: self.yluna_token.to_string(),
            vesting_period: self.vesting_period,
            boost_contract: self.boost_contract.to_string(),
            base_distribution_schedule: self.base_distribution_schedule,
            boost_distribution_schedule: self.boost_distribution_schedule,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    /// base_index is a snapshot of the global DISTRIBUTION_STATUS.reward_index field at the time of this user's
    /// previous bond/unbond event (see detailed example in reward_index documentation).
    pub base_index: Decimal,
    pub boost_index: Decimal,
    pub active_boost: Uint128,
    pub boost_weight: Uint128,
    /// pending_reward is the amount of PRISM tokens that already belong to the user (although they still need to go
    /// through the 30-day vesting period).
    pub pending_reward: Uint128,
}

impl RewardInfo {
    pub fn as_res(&self) -> RewardInfoResponse {
        RewardInfoResponse {
            base_index: self.base_index,
            boost_index: self.boost_index,
            boost_weight: self.boost_weight,
            pending_reward: self.pending_reward,
            active_boost: self.active_boost,
        }
    }
}
