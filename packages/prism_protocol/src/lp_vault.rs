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
    pub factory: String,
    pub collector: String,
    pub fee: Decimal,
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
        factory: Option<String>,
        collector: Option<String>,
        fee: Option<Decimal>,
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

    // unstake yLP
    Unstake { token: Addr,
              amount: Option<Uint128>, },

    // lets a user update their staking mode
    UpdateStakingMode { token: String,
                        mode: StakingMode, },

    // claims staked LP's rewards
    ClaimRewards { },

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
    CreateTokens { token: Addr,},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // LP -> cLP
    Bond { },

    // cLP -> LP
    Unbond { },

    // stake yLP to get rewards
    Stake { amount: Uint128, },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // owner of contract
    pub owner: String,

    // address of astroport generator
    pub generator: String,

    // address of astroport factory
    pub factory: String,

    // used to swap assets to prism
    pub collector: String,

    // prism fee of 15%
    pub fee: Decimal,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.to_string(),
            generator: self.generator.to_string(),
            factory: self.factory.to_string(),
            collector: self.collector.to_string(),
            fee: self.fee.clone(),
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub generator: String,
    pub factory: String,
    pub collector: String,
    pub fee: Decimal,
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