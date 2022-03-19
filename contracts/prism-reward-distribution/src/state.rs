use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_asset::AssetInfo;
use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;

pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELISTED_ASSETS: Item<Vec<AssetInfo>> = Item::new("whitelisted_assets");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub vault: Addr,
    pub gov: Addr,
    pub yasset_token: Addr,
    pub yasset_staking: Addr,
    pub yasset_staking_x: Addr,
    pub collector: Addr,
    pub protocol_fee: Decimal,
}
