use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;
use cw_storage_plus::Item;

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub yasset_token: Addr,
    pub xyasset_token: Addr,
    pub prism_token: Addr,
    pub collector: Addr,
    pub reward_distribution_contract: Option<Addr>,
}
