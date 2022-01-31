use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("The provided token is not available for staking")]
    InvalidStakingToken {},

    #[error("This address does not have staked tokens")]
    NothingStaked {},

    #[error("Cannot unbond more than bond amount")]
    InvalidUnbondAmount {},

    #[error("Current withdrawable amount is zero")]
    NothingAvailableToUnbond {},

    #[error("Invalid Cw20 msg")]
    InvalidCw20Msg {},

    #[error("Invalid distribution schedule")]
    InvalidDistributionSchedule {},

    #[error("Duplicate staking token")]
    DuplicateStakingToken {},

    #[error("The staking token is already registered")]
    AlreadyExists {},
}
