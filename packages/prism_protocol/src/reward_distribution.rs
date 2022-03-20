use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Decimal;
use cw_asset::AssetInfo;

pub const MAX_PROTOCOL_FEE: &str = "0.5";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub vault: String,
    pub collector: String,
    pub yasset_token: String,
    pub yasset_staking: String,
    pub yasset_staking_x: String,
    pub protocol_fee: Decimal,
    pub whitelisted_assets: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Distribute rewards to yasset_staking, yasset_staking_x, and collector.
    DistributeRewards {},

    ////////////////////////
    /// Owner operations
    ////////////////////////
    WhitelistRewardAsset {
        asset: AssetInfo,
    },

    RemoveRewardAsset {
        asset: AssetInfo,
    },

    UpdateConfig {
        owner: Option<String>,
        protocol_fee: Option<Decimal>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    RewardAssetWhitelist {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub vault: String,
    pub collector: String,
    pub yasset_token: String,
    pub yasset_staking: String,
    pub yasset_staking_x: String,
    pub protocol_fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardAssetWhitelistResponse {
    pub assets: Vec<AssetInfo>,
}
