use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw20::Cw20ReceiveMsg;
use std::fmt;

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
    Split {
        token: Addr,
        amount: Uint128,
    },

    // [p/y]LP -> cLP
    Merge {
        token: Addr,
        amount: Uint128,
    },

    // unstake yLP
    Unstake {
        token: Addr,
        amount: Option<Uint128>,
    },

    // lets a user update their staking mode
    UpdateStakingMode {
        token: Addr,
        mode: StakingMode,
    },

    // claims staked LP's rewards
    ClaimRewards {},

    ////////////////////
    /// internal operations
    ///////////////////

    // create a new set of c/p/y LP tokens given valid LP token
    CreateTokens {
        token: Addr,
    },

    // update LP rewards for all users staking this LP
    UpdateLPRewards {
        token: Addr,
    },

    // send all LP rewards to this staker
    SendStakerRewards {
        staker: Addr,
    },

    // update internal staker state
    UpdateStakerInfo {
        lp_id: u64,
        sender_addr: Addr,
        amount: Uint128,
        stake: bool,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // LP -> cLP
    Bond {},

    // cLP -> LP
    Unbond {},

    // user stakes yLP to get rewards
    Stake {},
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

    // used to swap assets to prism and accrue protocol fees
    pub collector: String,

    // prism fee of 15%
    pub fee: Decimal,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.clone(),
            generator: self.generator.clone(),
            factory: self.factory.clone(),
            collector: self.collector.clone(),
            fee: self.fee,
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
pub struct LPInfo {
    pub pair_asset_info: [AssetInfo; 2],
    pub generator_reward_info: Vec<AssetInfo>,
    pub amt_bonded: Uint128,
    pub last_liquidity: Decimal,
    pub pair_contract: Addr,
    pub lp_contract: Addr,
    pub clp_contract: Addr,
    pub plp_contract: Addr,
    pub ylp_contract: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub lp_contract: Addr,
    pub amt_staked: Uint128,
    pub mode: StakingMode,
    pub rewards: RewardInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    // index 0 = ASTRO, index 1 = proxy
    pub generator_rewards: Vec<Asset>,
    // order established by messages sent back from Astroport
    pub amm_rewards: Vec<Asset>,
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
