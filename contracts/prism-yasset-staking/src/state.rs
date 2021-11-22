use prism_protocol::yasset_staking::StakingMode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::AssetInfo;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELISTED_ASSETS: Item<Vec<AssetInfo>> = Item::new("whitelisted_assets");
pub const TOTAL_BOND_AMOUNT: Item<Uint128> = Item::new("total_bond_amount");

pub const POOL_INFO: Map<&[u8], PoolInfo> = Map::new("pool_info");
// owner, asset_info -> RewardInfo;
pub const REWARDS: Map<(&[u8], &[u8]), RewardInfo> = Map::new("rewards");
pub const BOND_AMOUNTS: Map<&[u8], BondInfo> = Map::new("bond_amounts");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub vault: Addr,
    pub gov: Addr,
    pub collector: Addr,
    pub reward_denom: String,
    pub protocol_fee: Decimal,
    pub cluna_token: Addr,
    pub yluna_token: Addr,
    pub pluna_token: Addr,
    pub prism_token: Addr,
    pub withdraw_fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct BondInfo {
    pub bond_amount: Uint128,
    pub mode: Option<StakingMode>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct PoolInfo {
    pub pending_reward: Uint128, // not distributed amount due to zero bonding
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub index: Decimal,
    pub pending_reward: Uint128,
}
