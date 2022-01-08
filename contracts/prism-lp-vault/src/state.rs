use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map, U64Key};

use prism_protocol::lp_vault::{Config, LPInfo, StakerInfo};

pub const CONFIG: Item<Config> = Item::new("config");

// map of [c/p/y/xy]LP -> unique id
pub const LP_IDS: Map<&Addr, u64> = Map::new("LP_ids");

// unique id -> LPInfo
pub const LP_INFOS: Map<U64Key, LPInfo> = Map::new("LP_infos");

// map of {unique id, user} -> StakerInfo
pub const STAKER_INFO: Map<(U64Key, &Addr), StakerInfo> = Map::new("staker_info");

// number of supported tokens
pub const NUM_LPS: Item<u64> = Item::new("num_lps");
