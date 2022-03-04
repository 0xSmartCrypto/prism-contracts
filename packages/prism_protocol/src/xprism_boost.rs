use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub xprism_token: String,
    pub boost_per_hour: Decimal,
    pub max_boost_per_xprism: Uint128,
    pub launch_pool_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),

    // owner operation
    UpdateConfig {
        owner: Option<String>,
        boost_per_hour: Option<Decimal>,
        max_boost_per_xprism: Option<Uint128>,
        launch_pool_contract: Option<String>,
    },

    // remove xprism
    Unbond {
        amount: Option<Uint128>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // add xprism
    Bond { user: Option<String> },
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
    /// boost_per_hour represents the amount of AMPS a user will accumulate per
    /// bound xPRISM per hour. 1 boost/hr = Decimal(1_000_000)
    pub boost_per_hour: Decimal,
    /// max amount of boost per xprism for a user, 6 decimal places (1 boost = 1_000_000)
    pub max_boost_per_xprism: Uint128,
    /// Address of the launch-pool contract. If set, this contract will be called when a user's AMPS go to zero to make
    /// sure the launch-pool contract stops accruing boosted rewards for this user.
    pub launch_pool_contract: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub amt_bonded: Uint128,  // amount of xprism
    pub total_boost: Uint128, // 6 decimal places
    pub last_updated: u64,    // seconds
    // time when first bond initially occurred, updated on a withdraw
    pub boost_accrual_start_time: u64, // seconds
}
