use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Reward asset not whitelisted: {asset}")]
    RewardAssetNotWhitelisted { asset: String },

    #[error("Duplicate whitelist asset: {asset}")]
    DuplicateWhitelistAsset { asset: String },

    #[error("Invalid protocol fee")]
    InvalidProtocolFee {},

    #[error("EmptyVault")]
    EmptyVault {},
}

pub type ContractResult<T> = core::result::Result<T, ContractError>;
