use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use prism_protocol::xprism_boost::{Config, UserInfo, UserInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const USER_INFO: Map<&[u8], UserInfo> = Map::new("user_info");
