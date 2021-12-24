use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env, Decimal, StdResult, Uint128, StdError};

use crate::error::ContractError;
use crate::state::{
    Config, RewardInfo, CONFIG,
};

use prism_protocol::lp_vault::{ConfigResponse};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(config.as_res()?)
}