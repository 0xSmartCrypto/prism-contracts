use cosmwasm_std::{Addr, Uint128};
use cw_asset::{Asset, AssetInfo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub distribution_contract: String, // collected rewards receiver
    pub astroport_factory: String,
    pub prismswap_factory: String,
    pub prism_token: String,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Any user can call convert to swap the asset tokens that collector holds
    /// for $PRISM, the resulting $PRISM is sent to distribution_contract
    Distribute { asset_infos: Vec<AssetInfo> },
    /// Any user can call ConvertAndSend to swap the provided assets to
    /// $PRISM and send to the reciver address (or sender if empty)
    /// Requires the sender to increase allowance for the requested assets
    ConvertAndSend {
        assets: Vec<Asset>,
        receiver: Option<String>,
    },
    /// Hook to swap base_denom for $PRISM,
    /// Called when there is not direct pair with requested asset_token
    /// Permissioned for internal calls only
    BaseSwapHook {
        receiver: Addr,
        prev_base_balance: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub distribution_contract: String, // collected rewards receiver
    pub astroport_factory: String,
    pub prismswap_factory: String,
    pub prism_token: String,
    pub base_denom: String,
}
