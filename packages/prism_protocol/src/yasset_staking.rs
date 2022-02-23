use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo};

pub const MAX_PROTOCOL_FEE: &str = "0.5";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub protocol_fee: Decimal,
    pub cluna_token: String,
    pub yluna_token: String,
    pub pluna_token: String,
    pub prism_token: String,
    pub xprism_token: String,
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

    /// Withdraw pending rewards and convert to a whitelisted asset info
    ConvertAndClaimRewards {
        claim_asset: AssetInfo,
    },
    MintXprismClaimHook {
        receiver: Addr,
        prev_balance: Uint128,
    },

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
    DepositMintedPylunaHook {
        prev_pluna_balance: Uint128,
        prev_yluna_balance: Uint128,
    },

    /// Deposit rewards to yLuna stakers
    DepositRewards {
        assets: Vec<Asset>,
    },

    ////////////////////////
    /// Owner operations
    ////////////////////////
    UpdateConfig {
        owner: Option<String>,
        collector: Option<String>,
        protocol_fee: Option<Decimal>,
    },
    WhitelistRewardAsset {
        asset: AssetInfo,
    },
    RemoveRewardAsset {
        asset: AssetInfo,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Bond yLuna to start receiving luna staking rewards
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo { asset_token: String },
    RewardInfo { staker_addr: String },
    RewardAssetWhitelist {},
    BondAmount {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub vault: String,
    pub gov: String,
    pub collector: String,
    pub protocol_fee: Decimal,
    pub cluna_token: String,
    pub yluna_token: String,
    pub pluna_token: String,
    pub prism_token: String,
    pub xprism_token: String,
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
    pub rewards: Vec<Asset>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardAssetWhitelistResponse {
    pub assets: Vec<AssetInfo>,
}
