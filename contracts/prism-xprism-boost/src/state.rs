use cw_storage_plus::{Item, Map};
use prism_protocol::xprism_boost::{Config, UserInfo};

type UserKey = [u8];

pub const CONFIG: Item<Config> = Item::new("config");
pub const USER_INFO: Map<&UserKey, UserInfo> = Map::new("user_info");
