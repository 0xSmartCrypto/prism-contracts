use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub gov: String,
    pub collector: String,
    pub yasset_token: String,
    pub prism_token: String,
    pub xprism_token: String,
    pub reward_distribution: String,
    pub claim_assets: Vec<AssetInfo> 
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

    /// Withdraw pending rewards and convert to the claim_asset 
    ConvertAndClaimRewards {
        claim_asset: AssetInfo,
    },
    MintXprismClaimHook {
        receiver: Addr,
        prev_balance: Uint128,
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
    },

    AddClaimAsset {
        asset: AssetInfo,
    },

    RemoveClaimAsset {
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
    BondAmount {},
    // State currently only contains BondAmount, so could just use BondAmount 
    // query, but adding for consistency with vault and yasset-staking-x
    State {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub gov: String,
    pub collector: String,
    pub yasset_token: String,
    pub prism_token: String,
    pub xprism_token: String,
    pub reward_distribution: String,
    pub claim_assets: Vec<AssetInfo> 
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
pub struct StateResponse {
    pub total_bond_amount: Uint128,
}
