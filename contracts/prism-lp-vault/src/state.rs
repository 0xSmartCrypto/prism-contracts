use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use prism_protocol::lp_vault::{Config, RewardInfo, StakingMode};
use cw_storage_plus::{Item, Map, U64Key};

pub const CONFIG: Item<Config> = Item::new("config");

// map of [c/p/y/xy]LP -> unique id
pub const LP_IDS: Map<&Addr, u64> = Map::new("LP_ids");

// unique id -> LPInfo
pub const LP_INFOS: Map<U64Key, LPInfo> = Map::new("LP_infos");

// map of {user, unique id} -> StakerInfo
pub const STAKER_INFO: Map<(&Addr, U64Key), StakerInfo> = Map::new("staker_info");

// number of supported tokens
pub const NUM_LPS: Item<u64> = Item::new("num_lps");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LPInfo {
    pub amt_bonded: Uint128,
    pub last_liquidity: Decimal,
    // swap to vec of Assets
    // store pair contract
    pub underlying_coin_denom_1: String,
    pub underlying_coin_denom_2: String,
    pub pair_contract: Addr,
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