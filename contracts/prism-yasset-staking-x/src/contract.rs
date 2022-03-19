#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, Fraction,
    MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};

use prism_protocol::internal::parse_reply_instantiate_data;
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::yasset_staking_x::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
};

use crate::error::{ContractError, ContractResult};
use crate::state::{Config, CONFIG};

use cw_asset::{Asset, AssetInfo};
use prismswap::querier::{query_supply, query_token_balance};
use prismswap::asset::{PrismSwapAsset};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse, TokenInfoResponse};
use cw20_base::msg::InstantiateMsg as TokenInstantiateMsg;

const INSTANTIATE_REPLY_ID: u64 = 1;

const CONTRACT_NAME: &str = "prism-yasset-staking-x";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONFIG.save(
        deps.storage,
        &Config {
            owner: info.sender,
            yasset_token: deps.api.addr_validate(&msg.yasset_token)?,
            xyasset_token: Addr::unchecked(""),
            prism_token: deps.api.addr_validate(&msg.prism_token)?,
            collector: deps.api.addr_validate(&msg.collector)?,
            reward_distribution_contract: None,
        },
    )?;

    let yasset_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: msg.yasset_token,
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
        }))?;

    Ok(Response::new().add_submessage(SubMsg {
        // Create LP token
        msg: WasmMsg::Instantiate {
            admin: None,
            code_id: msg.token_code_id,
            msg: to_binary(&TokenInstantiateMsg {
                name: "x".to_string() + &yasset_info.name,
                symbol: "x".to_string() + &yasset_info.symbol,
                decimals: yasset_info.decimals,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None,
                }),
                marketing: None,
            })?,
            funds: vec![],
            label: "".to_string(),
        }
        .into(),
        gas_limit: None,
        id: INSTANTIATE_REPLY_ID,
        reply_on: ReplyOn::Success,
    }))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> ContractResult<Response> {
    if msg.id != INSTANTIATE_REPLY_ID {
        return Err(ContractError::InvalidReplyId {});
    }
    let res = parse_reply_instantiate_data(msg).map_err(|_| ContractError::ParseReplyError {})?;
    let xyasset_token_addr = deps.api.addr_validate(&res.contract_address)?;
    CONFIG.update(deps.storage, |mut cfg| -> StdResult<_> {
        cfg.xyasset_token = xyasset_token_addr.clone();
        Ok(cfg)
    })?;

    Ok(Response::new().add_attribute("xyasset_token_addr", xyasset_token_addr))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::DepositRewards { assets } => deposit_rewards(deps, env, info, assets),
        ExecuteMsg::PostInitialize {
            reward_distribution_contract,
        } => post_initialize(deps, env, info, reward_distribution_contract),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> ContractResult<Response> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yasset token contract can execute this message
            if cfg.yasset_token != info.sender {
                return Err(ContractError::Unauthorized {});
            }

            bond(deps, env, cw20_msg.sender, cw20_msg.amount)
        }
        Cw20HookMsg::Unbond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yasset token contract can execute this message
            if cfg.xyasset_token != info.sender {
                return Err(ContractError::Unauthorized {});
            }

            unbond(deps, env, cw20_msg.sender, cw20_msg.amount)
        }
    }
}

pub fn bond(
    deps: DepsMut,
    env: Env,
    staker_addr: String,
    amount: Uint128,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    let state = _query_state(deps.as_ref(), env, &cfg)?;

    // can't use the exchange rate directly, we need to remove newly bonded amount
    let total_bond_amount = state
        .total_bond_amount
        .checked_sub(amount)
        .map_err(|x| -> StdError { x.into() })?;
    let exchange_rate = if total_bond_amount.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(state.xyasset_supply, total_bond_amount)
    };
    let xyasset_mint_amount = amount * exchange_rate;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.xyasset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: staker_addr.clone(),
                amount: xyasset_mint_amount,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "bond"),
            attr("staker_addr", staker_addr),
            attr("amount", amount.to_string()),
            attr("mint_amount", xyasset_mint_amount.to_string()),
        ]))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    staker_addr: String,
    amount: Uint128,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    let state = _query_state(deps.as_ref(), env, &cfg)?;
    let yasset_redeem_amount = state.exchange_rate.inv().unwrap() * amount;
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.xyasset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn { amount })?,
            funds: vec![],
        }))
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.yasset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: staker_addr.clone(),
                amount: yasset_redeem_amount,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "unbond"),
            attr("staker_addr", staker_addr),
            attr("amount", amount.to_string()),
            attr("redeem_amount", yasset_redeem_amount.to_string()),
        ]))
}

pub fn deposit_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.reward_distribution_contract.clone().unwrap() {
        return Err(ContractError::Unauthorized {});
    }

    // luna -> prism -> yasset
    // UST -> prism -> yasset
    // whitelisted airdrops: asset -> UST -> prism -> yasset
    let mut messages = vec![];

    for asset in &assets {
        match asset.info.clone() {
            AssetInfo::Native(..) => {
                asset
                    .assert_sent_native_token_balance(&info)
                    .map_err(|_| ContractError::InvalidNativeFunds {})?;
            }
            AssetInfo::Cw20(contract_addr) => {
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: env.contract.address.to_string(),
                        amount: asset.amount,
                    })?,
                    funds: vec![],
                }));
            }
        }
    }

    // build list of assets that we need to swap to yasset_token
    let assets_to_swap: Vec<Asset> = assets
        .into_iter()
        .filter(|asset| match asset.info.clone() {
            AssetInfo::Cw20(contract_addr) => {
                contract_addr != cfg.yasset_token
            }
            _ => true,
        })
        .collect();

    // convert to yasset token
    if !assets_to_swap.is_empty() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.collector.to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: assets_to_swap,
                receiver: None,
                dest_asset_info: AssetInfo::Cw20(cfg.yasset_token),
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "deposit_rewards"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps, env)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        yasset_token: cfg.yasset_token.to_string(),
        xyasset_token: cfg.xyasset_token.to_string(),
        prism_token: cfg.prism_token.to_string(),
        collector: cfg.collector.to_string(),
        reward_distribution_contract: cfg.reward_distribution_contract.map(|x| x.to_string()),
    })
}

pub fn query_state(deps: Deps, env: Env) -> StdResult<StateResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    _query_state(deps, env, &cfg)
}

pub fn _query_state(deps: Deps, env: Env, cfg: &Config) -> StdResult<StateResponse> {
    let yasset_balance = query_token_balance(
        &deps.querier,
        &cfg.yasset_token,
        &env.contract.address,
    )?;
    let xyasset_supply = query_supply(&deps.querier, &cfg.xyasset_token)?;
    let exchange_rate = if yasset_balance.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(xyasset_supply, yasset_balance)
    };

    Ok(StateResponse {
        total_bond_amount: yasset_balance,
        xyasset_supply,
        exchange_rate,
    })
}

pub fn post_initialize(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    reward_distribution_contract: String,
) -> ContractResult<Response> {
    let mut cfg = CONFIG.load(deps.storage)?;

    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    if cfg.reward_distribution_contract.is_some() {
        return Err(ContractError::DuplicatePostInitialize {});
    }
    cfg.reward_distribution_contract = Some(deps.api.addr_validate(&reward_distribution_contract)?);
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::default())
}
