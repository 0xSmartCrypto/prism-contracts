use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub vault_contract: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    FabricateClaim {
        airdrop_token: String,
        stage: u8,
        amount: Uint128,
        proof: Vec<String>,
    },
    UpdateConfig {
        owner: Option<String>,
        vault_contract: Option<String>,
    },
    AddAirdropInfo {
        airdrop_token: String,
        airdrop_info: AirdropInfo,
    },
    RemoveAirdropInfo {
        airdrop_token: String,
    },
    UpdateAirdropInfo {
        airdrop_token: String,
        airdrop_info: AirdropInfo,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    AirdropInfo {
        airdrop_token: Option<String>,
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AirdropInfo {
    pub airdrop_contract: String,
    pub claim_type: ClaimType,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    Generic,
}

impl fmt::Display for AirdropInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub vault_contract: String,
    pub airdrop_tokens: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AirdropInfoElem {
    pub airdrop_token: String,
    pub info: AirdropInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AirdropInfoResponse {
    pub airdrop_info: Vec<AirdropInfoElem>,
}
