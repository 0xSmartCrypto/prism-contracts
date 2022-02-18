use cosmwasm_std::StdError;
use thiserror::Error;

use cw_asset::AssetInfo;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Missing route for: {asset} -> {dest_asset}")]
    MissingRoute {
        asset: AssetInfo,
        dest_asset: AssetInfo,
    },

    #[error("DuplicateAssets")]
    DuplicateAssets {},

    #[error("LogicError: {msg}")]
    LogicError { msg: String },

    #[error("Not implemented")]
    NotImplemented {},
}
