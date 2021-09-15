use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use terraswap::asset::AssetInfo;

pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELISTED_ASSETS: Item<Vec<AssetInfo>> = Item::new("whitelisted_assets");
pub const TOTAL_BOND_AMOUNT: Item<Uint128> = Item::new("total_bond_amount");

pub const POOL_INFO: Map<&[u8], PoolInfo> = Map::new("pool_info");
// owner, asset_info -> RewardInfo;
pub const REWARDS: Map<(&[u8], &[u8]), RewardInfo> = Map::new("rewards");
pub const BOND_AMOUNTS: Map<&[u8], Uint128> = Map::new("bond_amounts");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub cluna_token: String,
    pub yluna_token: String,
    pub pluna_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    pub pending_reward: Uint128, // not distributed amount due to zero bonding
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub pending_reward: Uint128,
}
