use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Uint128};
use cw20::Cw20ReceiveMsg;
use crate::signed_decimal::SignedDecimal;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub xprism_token: String,
    pub boost_interval: SignedDecimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),

    UpdateConfig {
        owner: Option<String>,
        xprism_token: Option<String>,
        boost_interval: Option<SignedDecimal>,
    },

    // remove xprism
    Unbond {
        amount: Option<Uint128>,
    },

    // update boost of a user
    UpdateBoost { user: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // add xprism
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    GetBoost { user: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: String,
    pub xprism_token: String,
    pub boost_interval: SignedDecimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub amt_bonded: Uint128,
    pub total_boost: SignedDecimal,
    pub last_updated: u64,
}