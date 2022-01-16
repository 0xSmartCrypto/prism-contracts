use cosmwasm_std::{Addr};
use cw_storage_plus::{Item, Map};

use prism_protocol::lp_vault_factory::{AstroConfig, TerraswapConfig, Config, LPContracts};

pub const CONFIG: Item<Config> = Item::new("config");
// lp addr -> lp contracts
pub const VAULTS: Map<&Addr, LPContracts> = Map::new("vaults");

// used to instantiate all contracts
pub const TEMP_LP_INFO: Item<LPContracts> = Item::new("temp_lp_info");

// AMM configs
pub const ASTRO_CONFIG: Item<AstroConfig> = Item::new("astro_config");
pub const TERRASWAP_CONFIG: Item<TerraswapConfig> = Item::new("terraswap_config");