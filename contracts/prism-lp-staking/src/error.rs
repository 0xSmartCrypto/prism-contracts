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

    #[error("Invalid Cw20 msg")]
    InvalidCw20Msg {},

    #[error("Invalid distribution schedule")]
    InvalidDistributionSchedule {},
}
