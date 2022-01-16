use cw_storage_plus::Item;
use cosmwasm_std::Uint128;

use prism_protocol::astroport_lp_vault::{Config, LPInfo};

pub const CONFIG: Item<Config> = Item::new("config");
pub const LP_INFO: Item<LPInfo> = Item::new("LP_info");
pub const STATE: Item<Uint128> = Item::new("state");
