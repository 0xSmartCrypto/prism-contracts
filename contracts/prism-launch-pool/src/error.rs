use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid distribution schedule")]
    InvalidDistributionSchedule {},

    #[error("Invalid bond amount: {reason}")]
    InvalidBond { reason: String },

    #[error("Invalid unbond amount: {reason}")]
    InvalidUnbond { reason: String },

    #[error("Invalid claim withdrawn rewards: {reason}")]
    InvalidClaimWithdrawnRewards { reason: String },

    #[error("InvalidActivateBoost: {reason}")]
    InvalidActivateBoost { reason: String },

    #[error("Invalid base pool ratio")]
    InvalidBasePoolRatio {},
}
