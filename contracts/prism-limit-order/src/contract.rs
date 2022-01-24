#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::order::{cancel_order, execute_order, submit_order};
use crate::query::{query_config, query_last_order_id, query_order, query_orders};
use crate::state::{generate_pair_key, Config, CONFIG, LAST_ORDER_ID, PAIRS};
use prism_protocol::limit_order::{ExecuteMsg, InstantiateMsg, QueryMsg};
use prismswap::asset::AssetInfo;

const CONTRACT_NAME: &str = "prism-limit-order";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    assert_fee(msg.order_fee)?;
    assert_fee(msg.executor_fee_portion)?;

    let config = Config {
        base_denom: msg.base_denom,
        owner: info.sender,
        fee_collector_addr: deps.api.addr_validate(msg.fee_collector_addr.as_str())?,
        prism_token: deps.api.addr_validate(msg.prism_token.as_str())?,
        prism_ust_pair: deps.api.addr_validate(msg.prism_ust_pair.as_str())?,
        min_fee_value: msg.min_fee_value,
        order_fee: msg.order_fee,
        executor_fee_portion: msg.executor_fee_portion,
        excess_collactor_addr: deps.api.addr_validate(msg.excess_collector_addr.as_str())?,
    };

    CONFIG.save(deps.storage, &config)?;
    LAST_ORDER_ID.save(deps.storage, &0u64)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::AddPair {
            asset_infos,
            pair_addr,
        } => {
            let pair_addr: Addr = deps.api.addr_validate(&pair_addr)?;

            add_pair(deps, info, asset_infos, pair_addr)
        }
        ExecuteMsg::UpdateConfig {
            owner,
            fee_collector_addr,
            order_fee,
            min_fee_value,
            executor_fee_portion,
        } => update_config(
            deps,
            info,
            owner,
            fee_collector_addr,
            order_fee,
            min_fee_value,
            executor_fee_portion,
        ),
        ExecuteMsg::SubmitOrder {
            offer_asset,
            ask_asset,
        } => submit_order(deps, env, info, offer_asset, ask_asset),
        ExecuteMsg::CancelOrder { order_id } => cancel_order(deps, info, order_id),
        ExecuteMsg::ExecuteOrder { order_id } => execute_order(deps, info, order_id),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Order { order_id } => to_binary(&query_order(deps, order_id)?),
        QueryMsg::Orders {
            bidder_addr,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_orders(
            deps,
            bidder_addr,
            start_after,
            limit,
            order_by,
        )?),
        QueryMsg::LastOrderId {} => to_binary(&query_last_order_id(deps)?),
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    fee_collector_addr: Option<String>,
    order_fee: Option<Decimal>,
    min_fee_value: Option<Uint128>,
    executor_fee_portion: Option<Decimal>,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_validate(owner.as_str())?;
    }

    if let Some(fee_collector_addr) = fee_collector_addr {
        config.fee_collector_addr = deps.api.addr_validate(fee_collector_addr.as_str())?;
    }

    if let Some(order_fee) = order_fee {
        assert_fee(order_fee)?;
        config.order_fee = order_fee;
    }

    if let Some(min_fee_value) = min_fee_value {
        config.min_fee_value = min_fee_value
    }

    if let Some(executor_fee_portion) = executor_fee_portion {
        assert_fee(executor_fee_portion)?;
        config.executor_fee_portion = executor_fee_portion;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

pub fn add_pair(
    deps: DepsMut,
    info: MessageInfo,
    asset_infos: [AssetInfo; 2],
    pair_addr: Addr,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let prism_asset_info = AssetInfo::Token {
        contract_addr: config.prism_token,
    };
    if !asset_infos[0].equal(&prism_asset_info) && !asset_infos[1].equal(&prism_asset_info) {
        return Err(StdError::generic_err(
            "one of the assets has to be PRISM token",
        ));
    }

    let pair_key = generate_pair_key(&asset_infos);
    if PAIRS.may_load(deps.storage, &pair_key)?.is_some() {
        return Err(StdError::generic_err("pair already exists"));
    }

    PAIRS.save(deps.storage, &pair_key, &pair_addr)?;

    Ok(Response::new().add_attribute("action", "add_pair"))
}

fn assert_fee(fee: Decimal) -> StdResult<()> {
    if fee > Decimal::one() {
        return Err(StdError::generic_err("fee can not be bigger than 1.0"));
    }
    Ok(())
}
