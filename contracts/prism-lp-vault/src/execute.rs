#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr,
};

use prism_protocol::lp_vault::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode, 
};

use crate::state::{Config, RewardInfo, CONFIG, REWARD_INFO, LAST_COLLECTED};
use crate::query::{query_config, query_reward_info};

use astroport::asset::AssetInfo;
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;

pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    vault: Option<String>,
    gov: Option<String>,
    collector: Option<String>,
    collect_period: Option<u64>,
) -> StdResult<Response> {
    // only owner must be able to send this message.
    let conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut messages: Vec<SubMsg> = vec![];

    if let Some(creator) = owner {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.owner = creator;
            Ok(last_config)
        })?;
    }

    if let Some(v) = vault {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.vault = v;
            Ok(last_config)
        })?;
    }

    if let Some(g) = gov {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.gov = g;
            Ok(last_config)
        })?;
    }

    if let Some(c) = collector {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.collector = c;
            Ok(last_config)
        })?;
    }

    if let Some(interval) = collect_period {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.collect_period = interval;
            Ok(last_config)
        })?;
    }

    Ok(Response::new()
    .add_submessages(messages)
    .add_attributes(vec![attr("action", "update_config")]))
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mode: Option<StakingMode>,
) -> StdResult<Response> {
    Ok(Response::new())
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    Ok(Response::new())
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    Ok(Response::new())
}

pub fn calculate_fees(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    Ok(Response::new())
}

pub fn update_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    Ok(Response::new())
}