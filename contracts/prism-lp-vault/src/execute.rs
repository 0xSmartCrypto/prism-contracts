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

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    mode: Option<StakingMode>,
) -> StdResult<Response> {

    // check that the token is legit and safe
    // we will probably want to use cw20 wrapper for this(?) (bond)
    // use the valid addr thingy too
    // this should be extensible to any LP token on astroport
    // Q1. how to use cw20 wrapper with an arbitrary number of LP tokens? how to generalize?

    // try adding the LP token to an astro generator
    // if astro generator throws an error, throw an error and stop
    // Q2. how to use astro generator? what will it do with an LP token it doesn't know? - look at prism-astro-generator-proxy for inspiration

    ////// message is well formed and token is legit past this point //////
    
    // refract the amount the user has sent into a corresponding pLP token and yLP token
    // Q3. how to split an arbitrary cw20 LP token into its p/yLP tokens? for extensibility
    // store the LP token into our contract wallet

    ////// refracting has been done past this point //////

    // check for already existing RewardInfo for (user, token)
    // if it exists, add quantity of LP sent and update_rewards, update stakingmode iff provided, else just update_rewards
    // else, create new corresponding RewardInfo

    ////// internal state should be good at this point //////

    // send yLP and pLP to user

    Ok(Response::new())
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Option<Uint128>,
) -> StdResult<Response> {

    // check that {user, token} RewardInfo exists in our map and the amount is ok (if provided)

    // check that the token is legit and safe
    // we will probably want to use cw20 wrapper for this(?) (unbond)
    // use the valid addr thingy too
    // this should be extensible to any LP token on astroport

    // try withdrawing from astro generator

    // check that we can burn the relevant amount of p/yLP from the user

    // if the amount left after burn is 0 (or amount is not provided), call claim rewards and delete the instance of RewardInfo(?)

    // send back the proper amount of LP to user
    Ok(Response::new())
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
) -> StdResult<Response> {
    // check that {user, token} RewardInfo exists
    
    // check that token is valid and safe

    // call update_rewards (calculate rewards? do we even need to store?)

    // send back all rewards

    Ok(Response::new())
}

pub fn update_staking_mode(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    mode: StakingMode,
) -> StdResult<Response> {
    // check that {user, token} RewardInfo exists
    
    // check that token is valid and safe

    // update_rewards

    // update StakingMode
    Ok(Response::new())
}

pub fn calculate_fees(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: String,
    token: String,
) -> StdResult<Response> {
    // calculate relevant AMM fees
    // Q4. how to calculate collected AMM fees? is there some astroport query for it? or do we calc manually

    // withdraw and burn corresponding amount of LP tokens from astroport generator

    // place underlyings in contract wallet

    Ok(Response::new())
}

pub fn update_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: String,
    token: String,
) -> StdResult<Response> {
    // update rewardinfo of given {user, token} after calculating fees
    // instead of updating all RewardInfo every time rewards are collected from astro generator,
    // we can probably instead look at RewardInfo's last collected and Config's collection time
    // and figure out how many cycles of rewards it collected
    // still need to figure out how this amm fee can be calculated tho/how it works
    Ok(Response::new())
}