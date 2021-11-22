use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use astroport::asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub distribution_contract: String, // collected rewards receiver
    pub astroport_factory: String,
    pub prism_token: String,
    pub prism_base_pair: String, // astro pair $PRISM<>base_denom
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Any user can call convert to swap the asset tokens that collector holds
    /// for $PRISM, the resulting $PRISM is sent to distribution_contract
    Distribute {
        asset_tokens: Vec<String>,
    },
    /// Any user can call ConvertAndSend to swap the provided assets to 
    /// $PRISM and send to the reciver address (or sender if empty)
    /// Requires the sender to increase allowance for the requested assets
    ConvertAndSend {
        assets: Vec<Asset>,
        receiver: Option<String>,
    },
    /// Hook to swap base_denom for $PRISM,
    /// Called when there is not direct pair with requested asset_token
    BaseSwapHook {
        receiver: Option<String>,
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
    pub prism_token: String,
    pub prism_base_pair: String,
    pub base_denom: String,
}
