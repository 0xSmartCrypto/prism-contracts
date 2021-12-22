use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub reward_denom: String,
    pub protocol_fee: Decimal,
    pub cluna_token: String,
    pub yluna_token: String,
    pub pluna_token: String,
    pub prism_token: String,
    pub withdraw_fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ////////////////////////
    /// User operations
    ////////////////////////
    /// Unbond yLUNA
    Unbond {
        amount: Option<Uint128>,
    },
    /// Withdraw pending rewards
    ClaimRewards {},

    ////////////////////////
    /// Internal operations
    ////////////////////////
    /// Process delegator rewards swaps to luna
    /// and calls the internal hooks
    /// 1) Swap delegator rewards to luna
    /// 2) LunaToPyluna
    /// 3) DepositMintedPylunaHook
    ProcessDelegatorRewards {},
    LunaToPylunaHook {},
    DepositMintedPylunaHook {},

    /// Deposit rewards to yLuna stakers
    DepositRewards {
        assets: Vec<Asset>,
    },

    ////////////////////////
    /// Gov operations
    ////////////////////////
    WhitelistRewardAsset {
        asset: AssetInfo,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Bond yLuna to start receiving luna staking rewards
    Bond { mode: Option<StakingMode> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo { asset_token: String },
    RewardInfo { staker_addr: String },
    RewardAssetWhitelist {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub reward_denom: String,
    pub protocol_fee: Decimal,
    pub cluna_token: String,
    pub yluna_token: String,
    pub pluna_token: String,
    pub prism_token: String,
    pub withdraw_fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoResponse {
    pub asset_token: String,
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub staker_addr: String,
    pub staked_amount: Uint128,
    pub staking_mode: Option<StakingMode>,
    pub rewards: Vec<Asset>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardAssetWhitelistResponse {
    pub assets: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingMode {
    XPrism,
    Default,
}

impl fmt::Display for StakingMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
