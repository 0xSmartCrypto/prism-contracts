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

    #[error("Invalid Bond")]
    InvalidBond {},

    #[error("Invalid Unbond")]
    InvalidUnbond {},

    #[error("Invalid boost interval")]
    InvalidBoostInterval {},

    #[error("Invalid max boost")]
    InvalidMaxBoost {},
}
