use std::fmt;
use cosmwasm_std::{Binary, Decimal, Uint128, StdResult, Addr};
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
        generator: Option<String>,
        gov: Option<String>,
        collector: Option<String>,
    },

    ////////////////////
    /// User's operations
    ////////////////////

    Receive(Cw20ReceiveMsg),

    // cLP -> [p/y]LP
    Split { token: String,
            amount: Uint128, },

    // [p/y]LP -> cLP
    Merge { token: String,
            amount: Uint128, },

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
           token: Addr,
           amount: Uint128, },
    
    // burns cLP and updates internal state
    Burn { user: String,
           token: Addr,
           amount: Uint128, },

    // create a new set of c/p/y LP tokens given valid LP token
    CreateTokens { },

    // updates the rewards that each user can claim on every bond/unbond
    UpdateRewards { },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // LP -> cLP
    Bond { },

    // cLP -> LP
    Unbond { },
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
pub struct Config {
    pub owner: String,
    pub generator: String,
    pub gov: String,
    pub collector: String,
    pub collect_period: u64,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.to_string(),
            generator: self.generator.to_string(),
            gov: self.gov.to_string(),
            collector: self.collector.to_string(),
            collect_period: self.collect_period.clone(),
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub pending_underlying_reward_1: Uint128,
    pub pending_underlying_reward_2: Uint128,
    pub pending_underlying_astro: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub generator: String,
    pub gov: String,
    pub collector: String,
    pub collect_period: u64,
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