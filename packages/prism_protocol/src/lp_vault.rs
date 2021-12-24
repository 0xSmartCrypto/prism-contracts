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
    pub generator: String,
    pub gov: String,
    pub collector: String,
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
        generator: Option<String>,
        gov: Option<String>,
        collector: Option<String>,
    },

    ////////////////////
    /// User's operations
    ////////////////////

    Receive(Cw20ReceiveMsg),

    // cLP -> LP
    Unbond { token: String,
             amount: Option<Uint128> },

    // cLP -> [p/y]LP
    Split { amount: Uint128, },

    // [p/y]LP -> cLP
    Merge { amount: Uint128, },

    // stake yLP to get rewards
    Stake { amount: Uint128, },

    // unstake yLP
    Unstake { amount: Uint128, },

    // lets a user update their staking mode
    UpdateStakingMode { token: String,
                        mode: StakingMode, },

    ////////////////////
    /// internal operations
    ///////////////////
    
    // performs LP -> cLP conversion
    Mint { user: String,
           token: String,
           amount: Uint128, },
    
    // burns cLP and updates internal state
    Burn { user: String,
           token: String,
           amount: Uint128, },

    // updates the rewards that each user can claim on every bond/unbond
    UpdateRewards { },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // LP -> cLP
    Bond { },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},

    // {user, id} -> StakerInfoResponse
    //StakerInfo {},

    // {user, id} -> RewardInfoResponse
    // is needed if its already in StakerInfo? should we split the two?
    //RewardInfo{}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub generator: String,
    pub gov: String,
    pub collector: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfoResponse {
    // amt staked
    // staking mode
    // RewardInfo
}

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