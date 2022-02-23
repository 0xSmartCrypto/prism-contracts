use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use prism_protocol::xprism_boost::{Config, UserInfo};

pub const CONFIG: Item<Config> = Item::new("config");
pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("user_info");
