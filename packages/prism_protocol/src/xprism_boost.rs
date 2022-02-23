use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub xprism_token: String,
    pub boost_interval: Decimal,
    pub max_boost_per_xprism: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),

    UpdateConfig {
        owner: Option<String>,
        boost_interval: Option<Decimal>,
        max_boost_per_xprism: Option<Uint128>,
    },

    // remove xprism
    Unbond {
        amount: Option<Uint128>,
    },
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

    // updates user's boost lazily whenever requested
    GetBoost { user: Addr },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub xprism_token: Addr,
    // boost_interval represents the amount of amps a user will accumulate per xprism
    // per hour
    pub boost_interval: Decimal,
    pub max_boost_per_xprism: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub amt_bonded: Uint128,
    pub total_boost: Uint128, // 6 decimal places
    pub last_updated: u64,
    // time when first bond initially occurred, updated on a withdraw
    pub initial_bond: u64,
}
