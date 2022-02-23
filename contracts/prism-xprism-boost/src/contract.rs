use crate::error::ContractError;
use crate::state::{CONFIG, USER_INFO};
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Storage, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use prism_protocol::xprism_boost::{
    Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, UserInfo,
};
use std::cmp::min;
use std::str::FromStr;

const CONTRACT_NAME: &str = "prism-xprism-boost";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MAX_BOOST_PER_HOUR: &str = "1.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.boost_per_hour > Decimal::from_str(MAX_BOOST_PER_HOUR)? {
        return Err(ContractError::InvalidBoostInterval {});
    }

    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        xprism_token: deps.api.addr_validate(&msg.xprism_token)?,
        boost_per_hour: msg.boost_per_hour,
        max_boost_per_xprism: msg.max_boost_per_xprism,
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
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        _ => {
            let config = CONFIG.load(deps.storage)?;
            match msg {
                ExecuteMsg::Receive(msg) => {
                    // only xprism token contract can execute this message
                    if config.xprism_token != info.sender {
                        return Err(ContractError::Unauthorized {});
                    }
                    receive_cw20(deps, env, info, msg)
                }
                ExecuteMsg::UpdateConfig {
                    owner,
                    boost_per_hour,
                    max_boost_per_xprism,
                } => {
                    // only owner
                    if config.owner != info.sender {
                        return Err(ContractError::Unauthorized {});
                    }

                    update_config(deps, info, owner, boost_per_hour, max_boost_per_xprism)
                }
                _ => Err(ContractError::Unauthorized {}),
            }
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => bond(deps, env, &cw20_msg.sender, cw20_msg.amount),
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

/// Only owner can execute
pub fn update_config(
    deps: DepsMut,
    _info: MessageInfo,
    owner: Option<String>,
    boost_per_hour: Option<Decimal>,
    max_boost_per_xprism: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // owner update
    if let Some(owner) = owner {
        cfg.owner = deps.api.addr_validate(&owner)?;
    }

    // boost interval update
    if let Some(boost_per_hour) = boost_per_hour {
        if boost_per_hour > Decimal::from_str(MAX_BOOST_PER_HOUR)? {
            return Err(ContractError::InvalidBoostInterval {});
        }

        cfg.boost_per_hour = boost_per_hour;
    }

    // max xprism update
    if let Some(max_boost_per_xprism) = max_boost_per_xprism {
        // only allow increases
        if max_boost_per_xprism <= cfg.max_boost_per_xprism {
            return Err(ContractError::InvalidMaxBoost {});
        }
        cfg.max_boost_per_xprism = max_boost_per_xprism;
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new())
}

/// Any user can execute
pub fn bond(
    deps: DepsMut,
    env: Env,
    sender: &str,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidBond {});
    }

    let addr = deps.api.addr_validate(sender)?;
    let info = USER_INFO.load(deps.storage, &addr).unwrap_or(UserInfo {
        amt_bonded: Uint128::zero(),
        total_boost: Uint128::zero(),
        last_updated: env.block.time.seconds(),
        initial_bond: env.block.time.seconds(),
    });

    let mut user_info = _accumulate_boost(deps.storage, env, info)?;
    user_info.amt_bonded += amount;
    USER_INFO.save(deps.storage, &addr, &user_info)?;
    Ok(Response::new().add_attributes(vec![attr("user", addr), attr("bond", amount)]))
}

/// Any user that has previously bonded can execute
pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>, // If amount is None, the user's entire balance is unbonded
) -> Result<Response, ContractError> {
    let mut user_info = USER_INFO.load(deps.storage, &info.sender)?; // fails if does not exist
    let amt = amount.unwrap_or(user_info.amt_bonded);

    if amt > user_info.amt_bonded {
        return Err(ContractError::InvalidUnbond {});
    }

    user_info.amt_bonded = user_info.amt_bonded.checked_sub(amt)?;

    // By design, boost resets to 0 whenever a user unbonds any amount (no matter how small).
    user_info.total_boost = Uint128::zero();

    if user_info.amt_bonded.is_zero() {
        USER_INFO.remove(deps.storage, &info.sender);
    } else {
        user_info.last_updated = env.block.time.seconds();
        user_info.initial_bond = env.block.time.seconds();
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

/// Internal operation to calculate boost accrued since `last_updated` and accumulate it
/// into `total_boost`. `total_boost` should never exceed the maxmimum boost allowed.
pub fn _accumulate_boost(
    storage: &dyn Storage,
    env: Env,
    mut info: UserInfo,
) -> StdResult<UserInfo> {
    if !info.amt_bonded.is_zero() && env.block.time.seconds() > info.last_updated {
        let cfg = CONFIG.load(storage)?;
        let new_boost = info.amt_bonded
            * cfg.boost_per_hour
            * Decimal::from_ratio(
                (env.block.time.seconds() - info.last_updated) as u128,
                3600u128,
            );
        let max_boost = info.amt_bonded * cfg.max_boost_per_xprism;
        info.total_boost = min(info.total_boost + new_boost, max_boost);
        info.last_updated = env.block.time.seconds();
    }
    Ok(info)
}
