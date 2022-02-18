use crate::error::ContractError;
use crate::state::{CONFIG, USER_INFO};
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Response, StdResult, Storage, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use prism_protocol::xprism_boost::{
    Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, UserInfo,
};
use std::cmp::min;

const CONTRACT_NAME: &str = "prism-xprism-boost";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        xprism_token: deps.api.addr_validate(&msg.xprism_token)?,
        boost_interval: msg.boost_interval,
    };

    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            xprism_token,
            boost_interval,
        } => update_config(deps, info, owner, xprism_token, boost_interval),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only xprism token contract can execute this message
            if cfg.xprism_token != info.sender {
                return Err(ContractError::Unauthorized {});
            }
            bond(deps, env, &cw20_msg.sender, cw20_msg.amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetBoost { user } => {
            let info = USER_INFO.load(deps.storage, &user)?;
            to_binary(&_accumulate_boost(deps.storage, env, info)?)
        }
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<Addr>,
    xprism_token: Option<Addr>,
    boost_interval: Option<Decimal>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    cfg.owner = owner.unwrap_or(cfg.owner);
    cfg.xprism_token = xprism_token.unwrap_or(cfg.xprism_token);
    cfg.boost_interval = boost_interval.unwrap_or(cfg.boost_interval);
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new())
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    sender: &str,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(sender)?;
    let info = USER_INFO.load(deps.storage, &addr).unwrap_or(UserInfo {
        amt_bonded: Uint128::zero(),
        total_boost: Uint128::zero(),
        last_updated: env.block.time.seconds(),
    });

    let mut user_info = _accumulate_boost(deps.storage, env, info)?;
    user_info.amt_bonded += amount;
    USER_INFO.save(deps.storage, &addr, &user_info)?;
    Ok(Response::new().add_attributes(vec![attr("user", addr), attr("bond", amount)]))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut user_info = USER_INFO.load(deps.storage, &info.sender)?;
    let amt = amount.unwrap_or(user_info.amt_bonded);
    user_info.amt_bonded = user_info.amt_bonded.checked_sub(amt)?;
    user_info.total_boost = Uint128::zero();

    if user_info.amt_bonded.is_zero() {
        USER_INFO.remove(deps.storage, &info.sender);
    } else {
        user_info.last_updated = env.block.time.seconds();
        USER_INFO.save(deps.storage, &info.sender, &user_info)?;
    }

    let cfg = CONFIG.load(deps.storage)?;
    let messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.xprism_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.to_string(),
            amount: amt,
        })?,
        funds: vec![],
    })];

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("user", info.sender), attr("unbond", amt)]))
}

pub fn _accumulate_boost(
    storage: &dyn Storage,
    env: Env,
    mut info: UserInfo,
) -> StdResult<UserInfo> {
    if !info.amt_bonded.is_zero() && env.block.time.seconds() > info.last_updated {
        let cfg = CONFIG.load(storage)?;
        let new_boost = info.amt_bonded
            * cfg.boost_interval
            * Decimal::from_ratio(
                (env.block.time.seconds() - info.last_updated) as u128,
                3600u128,
            );
        let max_boost = info.amt_bonded * Uint128::from(100u128);
        info.total_boost = min(new_boost, max_boost);
        info.last_updated = env.block.time.seconds();
    }
    Ok(info)
}
