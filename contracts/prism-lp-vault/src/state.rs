use prism_protocol::lp_vault::{ConfigResponse, StakingMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");

// owner, asset_info -> RewardInfo;
pub const REWARD_INFO: Map<(&[u8], &[u8]), RewardInfo> = Map::new("reward_info");

// updated to the last time prism collected LP rewards from astroport
pub const LAST_COLLECTED: Item<u64> = Item::new("last_collected");

// may need to store most stuff in the future for relevant getters

// when user calls to claim rewards we can lazily calculate the reward amount then
// instead of storing the reward value and updating periodically


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: String,
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub collect_period: u64,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.to_string(),
            vault: self.vault.to_string(),
            gov: self.gov.to_string(),
            collector: self.collector.to_string(),
            collect_period: self.collect_period,
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub bond_amount: Uint128,
    pub last_received: u64, // we will lazily calculate the available rewards to be claimed when ClaimRewards is called by user

    // we will likely want to encapsulate this into its own reward data structure
    pub pending_xprism_reward: Uint128,
    pub pending_underlying_reward_1: Uint128,
    pub pending_underlying_reward_2: Uint128,
    pub pending_underlying_astro: Uint128,

    pub staking_mode: Option<StakingMode>,
}