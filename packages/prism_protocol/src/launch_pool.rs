use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::Asset;
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    // authorized to execute AdminWithdrawRewards
    pub owner: String,
    // authorized to execute WithdrawRewardsBulk
    pub operator: String,
    pub prism_token: String,
    pub xprism_token: String,
    pub yluna_staking: String,
    pub yluna_token: String,
    pub gov: String,
    /// vesting period in seconds
    pub vesting_period: u64,
    pub boost_contract: String,
    // start, end, amount of $PRISM to distribute
    // distribute linearly
    pub base_distribution_schedule: (u64, u64, Uint128),
    pub boost_distribution_schedule: (u64, u64, Uint128),
    // Attempts to bond less than this amount will result in an error.
    // Units: µ-yLunas.
    pub min_bond_amount: Uint128,
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
        amount: Option<Uint128>,
    },

    /// Updates the user's boost weight based on the current boost amount
    ActivateBoost {},

    /// Refreshes boost value for a specific user. Meant to be called by the
    /// Boost contract when somebody's AMPS go to 0.
    PrivilegedRefreshBoost {
        /// Address of account.
        account: String,
    },

    /// Withdraw $PRISM rewards
    /// Starts 30 day vesting period
    WithdrawRewards {},

    /// Start vesting period for many accounts in a single call. See
    /// documentation for the `withdraw_rewards_bulk` function for details.
    WithdrawRewardsBulk {
        /// Process up to `limit` accounts in this call. Can be tweaked to
        /// process more or less users depending on gas fees.
        limit: u64,
        /// Only consider accounts whose address is strictly larger than this
        /// field.
        start_after_address: Option<String>,
    },

    /// Claim rewards that have been previously withdrawn via WithdrawRewards
    /// or WithdrawRewardsBulk.  Only vested rewards are available to be
    /// withdrawn here.  The claim_type parameter specifies how the user
    /// would like to receive those rewards.
    ClaimWithdrawnRewards {
        claim_type: ClaimType,
    },

    /// Withdraw underlying rewards from yLUNA staking contract
    AdminWithdrawRewards {},

    /// Helper for AdminWithdrawRewards
    AdminSendWithdrawnRewards {
        original_balances: Vec<Asset>,
    },

    /// Allows admin to update contract's config.
    UpdateConfig {
        min_bond_amount: Option<Uint128>,
    },

    /// Hook to bond xprism with the boost contract.  This hook is invoked
    /// when a user calls ClaimWithdrawnRewards with ClaimType=Amps.
    BondWithBoostContractHook {
        receiver: Addr,
        prev_xprism_balance: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ClaimType {
    /// claim rewards as Prism
    Prism,

    /// claim rewards as Xprism (via a MintXPrism gov message)
    Xprism,

    /// convert rewards to Xprism (via a MintXprism gov message), which is
    /// then bonded with boost contract on behalf of the claiming user
    Amps,
}

impl fmt::Display for ClaimType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Bond yLuna to start receiving $PRISM rewards
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    DistributionStatus {},
    RewardInfo { staker_addr: String },
    VestingStatus { staker_addr: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub operator: String,
    pub prism_token: String,
    pub xprism_token: String,
    pub gov: String,
    pub yluna_staking: String,
    pub yluna_token: String,
    pub vesting_period: u64,
    pub boost_contract: String,
    pub base_distribution_schedule: (u64, u64, Uint128),
    pub boost_distribution_schedule: (u64, u64, Uint128),
    pub min_bond_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionStatusResponse {
    pub base: DistributionInfo,
    pub boost: DistributionInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfo {
    pub total_distributed: Uint128,
    pub total_weight: Uint128,
    pub pending_reward: Uint128,
    pub reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub bond_amount: Uint128,
    pub base_index: Decimal,
    pub boost_index: Decimal,
    pub boost_weight: Uint128,
    pub active_boost: Uint128,
    pub pending_reward: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct VestingStatusResponse {
    pub scheduled_vests: Vec<(u64, Uint128)>,
    pub withdrawable: Uint128,
}
