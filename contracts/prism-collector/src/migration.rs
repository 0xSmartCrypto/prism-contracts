use cosmwasm_std::{Addr, StdResult, Storage};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Config, CONFIG};

pub const LEGACY: Item<LegacyConfig> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub distribution_contract: Addr,
    pub astroport_factory: Addr,
    pub prismswap_factory: Addr,
    pub prism_token: Addr,
    pub base_denom: String,
}

pub fn migrate_config(storage: &mut dyn Storage, prismswap_router: Addr) -> StdResult<()> {
    let legacy_config: LegacyConfig = LEGACY.load(storage)?;
    let config = Config {
        distribution_contract: legacy_config.distribution_contract,
        astroport_factory: legacy_config.astroport_factory,
        prismswap_factory: legacy_config.prismswap_factory,
        prismswap_router,
        prism_token: legacy_config.prism_token,
        base_denom: legacy_config.base_denom,
    };

    CONFIG.save(storage, &config)?;
    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use cosmwasm_std::{testing::mock_dependencies, Api};

    use crate::{
        migration::{migrate_config, LegacyConfig, LEGACY},
        state::{Config, CONFIG},
    };

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(&[]);

        LEGACY
            .save(
                &mut deps.storage,
                &LegacyConfig {
                    distribution_contract: deps.api.addr_validate("collector0000").unwrap(),
                    astroport_factory: deps.api.addr_validate("astrofactory0000").unwrap(),
                    prismswap_factory: deps.api.addr_validate("factory0000").unwrap(),
                    prism_token: deps.api.addr_validate("prism0000").unwrap(),
                    base_denom: "uusd".to_string(),
                },
            )
            .unwrap();

        migrate_config(
            &mut deps.storage,
            deps.api.addr_validate("router0000").unwrap(),
        )
        .unwrap();

        let config: Config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(
            config,
            Config {
                distribution_contract: deps.api.addr_validate("collector0000").unwrap(),
                astroport_factory: deps.api.addr_validate("astrofactory0000").unwrap(),
                prismswap_factory: deps.api.addr_validate("factory0000").unwrap(),
                prismswap_router: deps.api.addr_validate("router0000").unwrap(),
                prism_token: deps.api.addr_validate("prism0000").unwrap(),
                base_denom: "uusd".to_string(),
            }
        )
    }
}
