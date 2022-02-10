use crate::error::ContractError;
use crate::state::{CONFIG, USER_INFO};
use std::str::{from_utf8};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Order, QueryRequest, Response, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use prism_protocol::xprism_boost::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, Config, UserInfo, UserInfoResponse,
};

const CONTRACT_NAME: &str = "prism-xprism-boost";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        owner: msg.owner,
        xprism_token: msg.xprism_token,
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
        ExecuteMsg::UpdateConfig { owner, xprism_token, boost_interval } => update_config(deps, env, info, owner, xprism_token, boost_interval),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::UpdateBoostIndex {} => update_boost_index(deps, env, info),
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
            to_binary(&USER_INFO.load(deps.storage, &user.as_bytes())?)
        }
        QueryMsg::GetAllBoosts {} => {
            let mut infos = vec![];
            for user in USER_INFO.keys(deps.storage, None, None, Order::Ascending) {
                let info = USER_INFO.load(deps.storage, &user)?;
                infos.push(
                    UserInfoResponse {
                        user: deps.api.addr_validate(from_utf8(&user)?)?,
                        amt_bonded: info.amt_bonded,
                        total_boost: info.total_boost,
                    }
                );
            }
            to_binary(&infos)
        }
    }
}

pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    xprism_token: Option<String>,
    boost_interval: Option<f64>,
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
    let mut user_info = USER_INFO.load(deps.storage, &sender.as_bytes())
                                 .unwrap_or(UserInfo {
                                    amt_bonded: Uint128::zero(),
                                    total_boost: 0.0,
                                    last_updated: env.block.time.seconds(),
                                 });
    user_info.amt_bonded += amount;
    USER_INFO.save(deps.storage, &sender.as_bytes(), &user_info)?;
    Ok(Response::new())
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut user_info = USER_INFO.load(deps.storage, &info.sender.as_bytes())?;
    let amt = amount.unwrap_or(user_info.amt_bonded);
    user_info.amt_bonded.checked_sub(amt);
    user_info.total_boost = 0.0;

    if user_info.amt_bonded.is_zero() {
        USER_INFO.remove(deps.storage, &info.sender.as_bytes());
    } else {
        USER_INFO.save(deps.storage, &info.sender.as_bytes(), &user_info)?;
    }

    Ok(Response::new())
}

pub fn update_boost_index(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let tmp: &mut dyn Storage = deps.storage;
    // do we need to set a max limit here?
    // make this better or do lazy eval per user
    for user in USER_INFO.keys(deps.storage, None, None, Order::Ascending) {
        let mut info = USER_INFO.load(tmp, &user)?;
        if env.block.time.seconds() > info.last_updated {
            info.total_boost += (cfg.boost_interval / 3600.0) * (env.block.time.seconds() - info.last_updated) as f64;
            info.last_updated = env.block.time.seconds();
        }
        USER_INFO.save(tmp, &user, &info)?;
    }
        
    Ok(Response::new())
}