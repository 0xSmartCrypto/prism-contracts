use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use prism_protocol::lp_vault::{Config, RewardInfo, StakingMode};
use cw_storage_plus::{Item, Map, U64Key};

pub const CONFIG: Item<Config> = Item::new("config");

// map of LP -> unique id
pub const LP_IDS: Map<&Addr, u64> = Map::new("LP_ids");
// map of cLP -> unique id
pub const CLP_IDS: Map<&Addr, u64> = Map::new("cLP_ids");
// map of pLP -> unique id
pub const PLP_IDS: Map<&Addr, u64> = Map::new("pLP_ids");
// map of yLP -> unique id
pub const YLP_IDS: Map<&Addr, u64> = Map::new("yLP_ids");
// xylp
// pub const xyLP_IDS: Map<&Addr, u64> = Map::new("xyLP_ids");

// number of supported tokens
pub const NUM_LPS: Item<u64> = Item::new("num_lps");

// unique id -> LPInfo
pub const LP_INFOS: Map<U64Key, LPInfo> = Map::new("LP_infos");

// map of {user, unique id} -> StakerInfo
pub const STAKER_INFO: Map<(&Addr, U64Key), StakerInfo> = Map::new("staker_info");

// need some form of collection period for ASTRO

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LPInfo {
    pub amt_bonded: Uint128,
    pub last_liquidity: Decimal,
    pub underlying_coin_denom_1: String,
    pub underlying_coin_denom_2: String,
    pub lp_addr: Addr,
    pub clp_addr: Addr,
    pub plp_addr: Addr,
    pub ylp_addr: Addr,
    // xylp
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub amt_bonded: Uint128,
    pub mode: StakingMode,
    pub reward_info: RewardInfo,
}