use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use terraswap::asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub vault: String,
    pub gov: String,
    pub yluna_token: String,
    pub cluna_token: String,
    pub prism_token: String,
    pub reward_denom: String,
    pub prism_pair: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ////////////////////////
    /// User operations ///
    ////////////////////////
    /// Unbond yLUNA
    Unbond {
        amount: Uint128,
    },
    /// Withdraw pending rewards
    Withdraw {},

    /// Private methods
    /// Before depositing delegator rewards, record the old reward denom
    /// balance, to get an accurate picture of what was gained
    UpdateRewardDenomBalance {

    },
    /// Swap delegator rewards into UST
    /// - Some UST goes towards yLUNA stakers
    /// - The rest is swapped into $PRISM and sent to Prism governance
    ProcessDelegatorRewards {

    },

    /// Everything has been swapped into $UST
    DepositRewardDenom {},

    /// deposit all $PRISM into gov contract
    DepositPrism {},

    /// Deposit rewards to yLuna stakers
    DepositRewards {
        assets: Vec<Asset>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Bond yLuna to start receiving luna staking rewards
    Bond {},
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo {
        asset_token: String,
    },
    RewardInfo {
        staker_addr: String,
    },
    Whitelist {

    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoResponse {
    pub asset_token: String,
    pub reward_index: Decimal,
    pub pending_reward: Uint128,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub staker_addr: String,
    pub reward_infos: Vec<RewardInfoResponseItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponseItem {
    pub asset_token: String,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}
