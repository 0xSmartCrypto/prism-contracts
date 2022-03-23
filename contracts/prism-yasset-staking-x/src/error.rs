use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InvalidNativeFunds")]
    InvalidNativeFunds {},

    #[error("InvalidReplyId")]
    InvalidReplyId {},

    #[error("ParseReplyError")]
    ParseReplyError {},
}

pub type ContractResult<T> = core::result::Result<T, ContractError>;
