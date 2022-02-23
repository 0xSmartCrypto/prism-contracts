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
    // boost_per_hour represents the amount of amps a user will accumulate per xprism
    // per hour. 1 boost/hr = Decimal(1000000)
    pub boost_per_hour: Decimal,
    // max amount of boost per xprism for a user, 6 decimal places (1 boost = 1000000)
    pub max_boost_per_xprism: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub amt_bonded: Uint128,  // amount of xprism
    pub total_boost: Uint128, // 6 decimal places
    pub last_updated: u64,    // seconds
    // boost_acrual_start_time is a timestamp in seconds. It is used in the UI
    // to show users how long they have been accruing AMPS for. It is updated to
    // the current time in the following situations:
    //   - On bond, when you used to have 0 bonded ylunas and now you have >0
    //     (but not on bond when you already had >0 bonded ylunas before)
    //   - Any time you unbond
    pub boost_accrual_start_time: u64,
}
