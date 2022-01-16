use cosmwasm_std::{Addr, Decimal, StdResult, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw20::Cw20ReceiveMsg;

use astroport::asset::{AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub generator: String,
    pub factory: String,
    pub fee: Decimal,

    pub lp_contract: String,
    pub clp_contract: String,
    pub plp_contract: String,
    pub ylp_contract: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Set the owner
    UpdateConfig {
        owner: Option<Addr>,
        generator: Option<Addr>,
        factory: Option<Addr>,
        reward_dist: Option<Addr>,
        fee: Option<Decimal>,
    },

    ////////////////////
    /// User's operations
    ////////////////////
    Receive(Cw20ReceiveMsg),

    // cLP -> LP
    Unbond { amount: Uint128 },

    // cLP -> [p/y]LP
    Split {
        amount: Uint128,
    },

    // [p/y]LP -> cLP
    Merge {
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // LP -> cLP
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // owner of contract
    pub owner: Addr,

    // address of astroport generator
    pub generator: Addr,

    // address of astroport factory
    pub factory: Addr,

    // used to distribute rewards to stakers and protocol
    pub reward_dist: Addr,

    // prism LP fee of 15%
    pub fee: Decimal,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.clone().into_string(),
            generator: self.generator.clone().into_string(),
            factory: self.factory.clone().into_string(),
            reward_dist: self.reward_dist.clone().into_string(),
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
    pub reward_dist: String,
    pub fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LPInfo {
    pub pair_asset_info: [AssetInfo; 2],
    pub generator_reward_info: Vec<AssetInfo>,
    pub amt_lp: Uint128,
    pub amt_clp: Uint128,
    pub last_liquidity: Decimal,
    pub pair_contract: Addr,
    pub lp_contract: Addr,
    pub clp_contract: Addr,
    pub plp_contract: Addr,
    pub ylp_contract: Addr,
}
