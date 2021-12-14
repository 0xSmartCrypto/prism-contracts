use std::fmt;
use cosmwasm_std::{Binary, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// figure out how to use this
use astroport::asset::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub collect_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ////////////////////
    /// Owner's operations
    ////////////////////

    // Set the owner
    UpdateConfig {
        owner: Option<String>,
        vault: Option<String>,
        gov: Option<String>,
        collector: Option<String>,
        collect_period: Option<u64>,
    },


    ////////////////////
    /// User's operations
    ////////////////////

    // some of these should be in Cw20ReceiveMsg interfaces prob
    // and only called by the asset token

    // Receives some amount of cw20 LP token from user
    // Attempts to put the LP token into an astro generator
    // On successful attempt, mints [y/p]LP and issues to user
    Bond { mode: Option<StakingMode>,  },

    // unbonds 
    Unbond { amount: Option<Uint128>, },

    // withdraw rewards
    ClaimRewards {},

    ////////////////////
    /// internal operations
    ///////////////////
    
    // runs on a fixed schedule (collect_period)
    // calculates AMM fees since last collection
    // burns corresponding number of LP tokens
    CalculateFees { },

    // updates the rewards that each user can claim via ClaimRewards
    UpdateRewards { },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},

    RewardInfo { 
        stakerAddr: String, 
    },
    WithdrawableUnbonded { 
        address: String, 
    },

    // build this out later if needed
    // StakerInfo {
    //     staker: String,
    //     staking_token: Option<String>,
    // },
    // TokenStakersInfo {
    //     staking_token: String,
    //     start_after: Option<String>,
    //     limit: Option<u32>,
    // },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub collect_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub bond_amount: Uint128,
    pub last_received: u64,
}

// build these out later if needed
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct StakerInfoResponse {
//     // return for a specific 
// }

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct TokenStakersInfoResponse {
//     //WIP
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingMode {
    Default,
    XPrism,
    Autocompound, // WIP
}

impl fmt::Display for StakingMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}