use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");

pub const POOL_INFO: Map<&[u8], PoolInfo> = Map::new("pool_info");
// owner, asset_info -> RewardInfo;
pub const REWARDS: Map<(&[u8], &[u8]), RewardInfo> = Map::new("rewards");
pub const BOND_AMOUNTS: Map<&[u8], BondInfo> = Map::new("bond_amounts");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub yasset_token: Addr,
    pub reward_distribution_contract: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct BondInfo {
    pub bond_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct PoolInfo {
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub index: Decimal,
    pub pending_reward: Uint128,
}
