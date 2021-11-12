use cosmwasm_std::Uint128;
use cw_storage_plus::{Item, Map};
use prism_protocol::fair_launch::LaunchConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");

pub const TOTAL_TOKEN: Item<Uint128> = Item::new("total_token");
pub const TOTAL_DEPOSIT: Item<Uint128> = Item::new("total_deposit");
pub const DEPOSITS: Map<&[u8], Uint128> = Map::new("deposits");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: String,
    pub token: String,
    pub launch_config: Option<LaunchConfig>,
}
