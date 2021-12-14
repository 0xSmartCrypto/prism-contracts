use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env, Decimal, StdResult, Uint128, StdError};

use crate::error::ContractError;
use crate::state::{
    Config, RewardInfo, CONFIG, REWARD_INFO,
};

use prism_protocol::lp_vault::{ConfigResponse};

pub fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(config.as_res()?)
}

pub fn query_reward_info(deps: Deps, staker: Addr, staking_token: Addr) -> StdResult<RewardInfo> {
    let staker_raw: CanonicalAddr = deps.api.addr_canonicalize(staker.as_str())?;
    let staking_token_raw: CanonicalAddr = deps.api.addr_canonicalize(staking_token.as_str())?;
    let reward_info: RewardInfo = REWARD_INFO.load(deps.storage, (staker_raw.as_slice(), staking_token_raw.as_slice()))
                                             .map_err(|_| StdError::generic_err("there is no reward info for this token"))?;


    Ok(reward_info)
}