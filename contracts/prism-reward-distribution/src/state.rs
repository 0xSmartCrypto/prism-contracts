use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal};
use cw_asset::AssetInfo;
use cw_storage_plus::Item;

use prism_protocol::reward_distribution::ConfigResponse;

use crate::error::{ContractError, ContractResult};

pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELISTED_ASSETS: Item<Vec<AssetInfo>> = Item::new("whitelisted_assets");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub vault: Addr,
    pub collector: Addr,
    pub yasset_token: Addr,
    pub yasset_staking: Addr,
    pub yasset_staking_x: Addr,
    pub protocol_fee: Decimal,
    pub initialized: bool,
}

impl Config {
    pub fn as_res(&self) -> ConfigResponse {
        ConfigResponse {
            owner: self.owner.to_string(),
            vault: self.vault.to_string(),
            collector: self.collector.to_string(),
            yasset_token: self.yasset_token.to_string(),
            yasset_staking: self.yasset_staking.to_string(),
            yasset_staking_x: self.yasset_staking_x.to_string(),
            protocol_fee: self.protocol_fee,
            initialized: self.initialized,
        }
    }

    pub fn assert_initialized(self) -> ContractResult<Config> {
        if self.initialized {
            Ok(self)
        } else {
            Err(ContractError::NotInitialized {})
        }
    }
}
