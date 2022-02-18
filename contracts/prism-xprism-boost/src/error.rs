use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Overflow")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid distribution schedule")]
    InvalidDistributionSchedule {},

    #[error("Invalid unbond amount: {reason}")]
    InvalidUnbond { reason: String },

    #[error("Invalid claim withdrawn rewards: {reason}")]
    InvalidClaimWithdrawnRewards { reason: String },
}
