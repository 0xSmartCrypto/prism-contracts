use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid Cw20 msg")]
    InvalidCw20Msg {},

    #[error("This message does not accept funds")]
    NonPayable {},

    #[error("Contract is not fully initialized")]
    NotInitialized {},

    #[error("Duplicate update config")]
    DuplicateUpdateConfig {},

    #[error("Invalid reply id")]
    InvalidReplayId {},

    #[error("Parse reply erro")]
    ParseReplyError {},
}

pub type ContractResult<T> = core::result::Result<T, ContractError>;
