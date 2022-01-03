use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use prism_protocol::lp_vault::{Config, StakingMode};
use cw_storage_plus::{Item, Map, U64Key};

pub const CONFIG: Item<Config> = Item::new("config");

// map of [c/p/y/xy]LP -> unique id
pub const LP_IDS: Map<&Addr, u64> = Map::new("LP_ids");

// unique id -> LPInfo
pub const LP_INFOS: Map<U64Key, LPInfo> = Map::new("LP_infos");

// map of {unique id, user} -> StakerInfo
pub const STAKER_INFO: Map<(U64Key, &Addr), StakerInfo> = Map::new("staker_info");

// number of supported tokens
pub const NUM_LPS: Item<u64> = Item::new("num_lps");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LPInfo {
    // calc
    pub pair_asset_info: [AssetInfo; 2],
    pub generator_reward_info: Vec<AssetInfo>,
    pub amt_bonded: Uint128,
    pub last_liquidity: Decimal,

    // contracts
    pub pair_contract: Addr,
    pub lp_contract: Addr,
    pub clp_contract: Addr,
    pub plp_contract: Addr,
    pub ylp_contract: Addr,
    // xylp
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub amt_staked: Uint128,
    pub mode: StakingMode,
    pub rewards: RewardInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    // will need to make this more extensible in the future for other AMMs, astroport specific for now

    // index 0 = ASTRO, index 1 = proxy
    pub generator_rewards: Vec<Asset>,
    
    // order established by messages sent back from Astroport
    pub amm_rewards: Vec<Asset>,
}