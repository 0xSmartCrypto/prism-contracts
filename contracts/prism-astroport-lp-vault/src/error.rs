use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("LP vault does not exist")]
    DoesNotExist {},

    #[error("LP vault already exists")]
    AlreadyExists {},

    #[error("AMM not supported")]
    AmmNotSupported {},

    #[error("Failed to parse reply")]
    ParseError {},

    #[error("Invalid reply ID")]
    InvalidReplyID {},

    #[error("Reply error")]
    ReplyErr {},
}

pub type ContractResult<T> = core::result::Result<T, ContractError>;
