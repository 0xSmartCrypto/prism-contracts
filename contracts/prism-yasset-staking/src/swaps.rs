use crate::state::CONFIG;
use cosmwasm_std::{
    attr, to_binary, Addr, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, SubMsg, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::yasset_staking::ExecuteMsg;
use terra_cosmwasm::{create_swap_msg, ExchangeRatesResponse, TerraMsgWrapper, TerraQuerier};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::{query_balance, query_token_balance};

/// Swap all native tokens to reward_denom
/// Only hub_contract is allowed to execute

pub fn process_delegator_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != cfg.vault {
        return Err(StdError::generic_err("unauthorized"));
    }

    let contr_addr = env.contract.address.clone();
    let balance = deps.querier.query_all_balances(contr_addr)?;
    let mut messages: Vec<SubMsg<TerraMsgWrapper>> = Vec::new();

    let reward_denom = "uluna".to_string();

    let mut is_listed = true;

    let denoms: Vec<String> = balance.iter().map(|item| item.denom.clone()).collect();

    if query_exchange_rates(&deps, reward_denom.clone(), denoms).is_err() {
        is_listed = false;
    }

    for coin in balance.clone() {
        if coin.denom == reward_denom.clone() {
            continue;
        }

        if is_listed {
            messages.push(SubMsg::new(create_swap_msg(coin, reward_denom.to_string())));
        } else if query_exchange_rates(&deps, reward_denom.clone(), vec![coin.denom.clone()])
            .is_ok()
        {
            messages.push(SubMsg::new(create_swap_msg(coin, reward_denom.to_string())));
        }
    }

    let res = Response::new()
        .add_submessages(messages)
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.clone().to_string(),
                msg: to_binary(&ExecuteMsg::LunaToCluna {})?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.clone().to_string(),
                msg: to_binary(&ExecuteMsg::ConvertAndDepositCluna {})?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![attr("action", "process_delegator_rewards")]);

    Ok(res)
}

pub fn luna_to_cluna(deps: DepsMut, env: Env) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    let luna_amt = query_balance(&deps.querier, env.contract.address, "uluna".to_string())?;
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.vault,
            msg: to_binary(&VaultExecuteMsg::Bond { validator: None })?,
            funds: vec![Coin {
                denom: "uluna".to_string(),
                amount: luna_amt,
            }],
        }))
        .add_attributes(vec![attr("action", "luna_to_cluna")]))
}

pub fn convert_and_deposit_cluna(deps: DepsMut, env: Env) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    let cluna_amt = query_token_balance(
        &deps.querier,
        Addr::unchecked(cfg.cluna_token.clone()),
        env.contract.address.clone(),
    )?;
    Ok(Response::new()
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.cluna_token.clone(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: cfg.vault.clone(),
                    amount: cluna_amt.clone(),
                    expires: None,
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.vault,
                msg: to_binary(&VaultExecuteMsg::Split { amount: cluna_amt })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.clone().to_string(),
                msg: to_binary(&ExecuteMsg::DepositRewards {
                    assets: vec![
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: cfg.yluna_token.clone(),
                            },
                            amount: cluna_amt.clone(),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: cfg.pluna_token.clone(),
                            },
                            amount: cluna_amt.clone(),
                        },
                    ],
                })?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![attr("action", "convert_and_deposit_cluna")]))
}

pub fn query_exchange_rates(
    deps: &DepsMut,
    base_denom: String,
    quote_denoms: Vec<String>,
) -> StdResult<ExchangeRatesResponse> {
    let querier = TerraQuerier::new(&deps.querier);
    let res: ExchangeRatesResponse = querier.query_exchange_rates(base_denom, quote_denoms)?;
    Ok(res)
}
