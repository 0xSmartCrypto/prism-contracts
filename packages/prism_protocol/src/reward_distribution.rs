use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal};
use cw_asset::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub vault: String,
    pub gov: String,
    pub yasset_token: String,
    pub yasset_staking: String,
    pub yasset_staking_x: String,
    pub collector: String,
    pub protocol_fee: Decimal,
    pub whitelisted_assets: Vec<AssetInfo> 
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Process delegator rewards, swap to reward denom
    ProcessDelegatorRewards {},
    
    /// Distribute entire balance of asset_infos as rewards to yasset_staking, 
    /// yasset_staking_x, and collector
    DistributeRewards {
        asset_infos: Vec<AssetInfo>,
    },
    
    ////////////////////////
    /// Gov operations
    ////////////////////////
    WhitelistRewardAsset {
        asset: AssetInfo,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    RewardAssetWhitelist {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub vault: String,
    pub gov: String,
    pub yasset_token: String,
    pub yasset_staking: String,
    pub yasset_staking_x: String,
    pub collector: String,
    pub protocol_fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardAssetWhitelistResponse {
    pub assets: Vec<AssetInfo>,
}
