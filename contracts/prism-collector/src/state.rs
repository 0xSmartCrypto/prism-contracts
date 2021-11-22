use prism_protocol::collector::ConfigResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, StdResult};
use cw_storage_plus::Item;

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub distribution_contract: Addr, // collected rewards receiver
    pub astroport_factory: Addr,
    pub prism_token: Addr,
    pub prism_base_pair: Addr,
    pub base_denom: String,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            prism_token: self.prism_token.to_string(),
            distribution_contract: self.distribution_contract.to_string(),
            astroport_factory: self.astroport_factory.to_string(),
            prism_base_pair: self.prism_base_pair.to_string(),
            base_denom: self.base_denom.clone(),
        };
        Ok(res)
    }
}
