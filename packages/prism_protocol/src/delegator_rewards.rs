use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub vault: String,
    pub yluna_token: String,
    pub pluna_token: String,
    pub reward_distribution: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {

    // Process delegator rewards, swap to luna
    ProcessDelegatorRewards {},

    // bond split luna with vault, receive pluna/yluna
    LunaToPylunaHook {},

    // send pluna/yluna as rewards to reward distribution contract 
    DistributeMintedPylunaHook {},

    UpdateConfig {
        owner: Option<String>
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub vault: String,
    pub yluna_token: String,
    pub pluna_token: String,
    pub reward_distribution: String,
}