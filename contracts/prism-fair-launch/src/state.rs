use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use cw_storage_plus::{Item, Map};
use prism_protocol::fair_launch::{ConfigResponse, LaunchConfig};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");

pub const TOTAL_TOKEN: Item<Uint128> = Item::new("total_token");
pub const TOTAL_DEPOSIT: Item<Uint128> = Item::new("total_deposit");
pub const DEPOSITS: Map<&Addr, Uint128> = Map::new("deposits");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub token: Addr,
    pub launch_config: Option<LaunchConfig>,
    pub base_denom: String,
    pub withdraw_fee: Decimal,
    pub withdraw_threshold: Uint128,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.to_string(),
            token: self.token.to_string(),
            launch_config: self.launch_config.clone(),
            base_denom: self.base_denom.clone(),
            withdraw_fee: self.withdraw_fee,
            withdraw_threshold: self.withdraw_threshold,
        };
        Ok(res)
    }
}
