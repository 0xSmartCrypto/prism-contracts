use prism_protocol::lp_vault::{ConfigResponse, StakingMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");

// some way to make these mappings more compact?
// map of LP -> uint
// map of cLP -> uint
// map of pLP -> uint
// map of yLP -> uint
// map of uint -> LPInfo = {LP addr, cLP addr, pLP addr, yLP addr, [xyLP addr]}

// map of {user, uint} -> StakerInfo

// item of last liquidity per LP

// to propogate rewards, calculate amt to deposit PER LP, iterate thru each relevant StakerInfo and add rewards via stakingmode


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: String,
    pub generator: String,
    pub gov: String,
    pub collector: String,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.to_string(),
            generator: self.generator.to_string(),
            gov: self.gov.to_string(),
            collector: self.collector.to_string(),
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct StakerInfo {
    // amt staked
    // staking mode
    // RewardInfo
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    // pending_astro
    // pending_underlying_1
    // pending_underlying_2
    // pending_xprism




    // old old old below

    pub bond_amount: Uint128,
    pub last_received: u64, // we will lazily calculate the available rewards to be claimed when ClaimRewards is called by user

    // we will likely want to encapsulate this into its own reward data structure
    pub pending_xprism_reward: Uint128,
    pub pending_underlying_reward_1: Uint128,
    pub pending_underlying_reward_2: Uint128,
    pub pending_underlying_astro: Uint128,

    pub staking_mode: Option<StakingMode>,
}