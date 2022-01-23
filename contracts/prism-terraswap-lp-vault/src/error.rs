use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("LP does not exist")]
    DoesNotExist {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Bad LP bonding amount")]
    BadBondAmount {},

    #[error("Bad LP unbonding amount")]
    BadUnbondAmount {},
}

pub type ContractResult<T> = core::result::Result<T, ContractError>;
