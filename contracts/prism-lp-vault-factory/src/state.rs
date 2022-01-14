use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map, U64Key};

use prism_protocol::lp_vault_factory::{Config, AstroConfig, LPContracts};

pub const CONFIG: Item<Config> = Item::new("config");
// amm id, lp addr -> lp contracts
pub const VAULTS: Map<(U64Key, &Addr), LPContracts> = Map::new("vaults");
pub const ASTRO_CONFIG: Item<AstroConfig> = Item::new("astro_config");

// used to instantiate all contracts
pub const TEMP_LP_INFO: Item<LPContracts> = Item::new("temp_lp_info");