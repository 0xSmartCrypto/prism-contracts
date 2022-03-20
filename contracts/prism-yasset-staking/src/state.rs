use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_asset::AssetInfo;
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");

/// TOTAL_BOND_AMOUNT holds the total amount of y-asset that has been staked by
/// people in this contract. It starts at 0. It is incremented during Bond calls
/// and decremented during Unbond calls.
pub const TOTAL_BOND_AMOUNT: Item<Uint128> = Item::new("total_bond_amount");

pub const POOL_INFO: Map<&[u8], PoolInfo> = Map::new("pool_info");
// owner, asset_info -> RewardInfo
pub const REWARDS: Map<(&[u8], &[u8]), RewardInfo> = Map::new("rewards");

/// BOND_AMOUNTS is a map detailing how much y-asset each user has staked in
/// this contract. It starts as an empty map. When a user stakes a quantity for
/// the first time, a new entry is inserted. This entry is never removed and is
/// kept up-to-date when Bond/Unbond are called.
///
///  Keys: owner address, Values: BondInfo
pub const BOND_AMOUNTS: Map<&[u8], BondInfo> = Map::new("bond_amounts");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub gov: Addr,
    pub collector: Addr,
    pub yasset_token: Addr,
    pub prism_token: Addr,
    pub xprism_token: Addr,
    pub reward_distribution: Addr,
    pub claim_assets: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
/// BondInfo has info about what a specific user has staked in this contract.
/// There is one BondInfo per user (stored in the BOND_AMOUNTS map).
pub struct BondInfo {
    pub bond_amount: Uint128, // amount of y-asset that was staked.
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct PoolInfo {
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
/// RewardInfo has info about rewards in a specific denomination that a specific
/// user has earned. There is one RewardInfo per (user, reward denomination)
/// pair (stored in the REWARDS map).
pub struct RewardInfo {
    pub index: Decimal,
    /// pending_reward is the amount of a specific asset that a user would
    /// immediately receive if he were to call ClaimRewards right now.
    pub pending_reward: Uint128,
}
