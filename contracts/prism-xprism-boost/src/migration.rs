use crate::state::CONFIG;
use cosmwasm_std::{Addr, Decimal, StdResult, Storage, Uint128};
use cw_storage_plus::Item;
use prism_protocol::xprism_boost::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const LEGACY: Item<LegacyConfig> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
struct LegacyConfig {
    pub owner: Addr,
    pub xprism_token: Addr,
    pub boost_per_hour: Decimal,
    pub max_boost_per_xprism: Uint128,
}

pub fn migrate_config(storage: &mut dyn Storage) -> StdResult<()> {
    let legacy_config: LegacyConfig = LEGACY.load(storage)?;
    let config = Config {
        owner: legacy_config.owner,
        xprism_token: legacy_config.xprism_token,
        boost_per_hour: legacy_config.boost_per_hour,
        max_boost_per_xprism: legacy_config.max_boost_per_xprism,
        launch_pool_contract: None,
    };

    CONFIG.save(storage, &config)?;
    Ok(())
}
