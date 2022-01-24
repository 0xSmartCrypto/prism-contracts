use crate::state::CONFIG;
use cosmwasm_std::{
    attr, to_binary, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::yasset_staking::ExecuteMsg;
use prismswap::asset::{Asset, AssetInfo};
use prismswap::querier::{query_balance, query_token_balance};
use terra_cosmwasm::{create_swap_msg, ExchangeRatesResponse, TerraMsgWrapper, TerraQuerier};

pub const REWARD_DENOM: &str = "uluna";

/// 1. Swap all native tokens to uluna
/// 2. Use the uluna to mint pluna and yluna
/// 4. Deposit pluna and yluna as reward to stakers
pub fn process_delegator_rewards(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let contr_addr = env.contract.address.clone();
    let balances = deps.querier.query_all_balances(contr_addr)?;
    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = Vec::new();

    let reward_denom = String::from(REWARD_DENOM);

    let denoms: Vec<String> = balances.iter().map(|item| item.denom.clone()).collect();

    let exchange_rates = query_exchange_rates(&deps, reward_denom.clone(), denoms)?;

    for coin in balances {
        if coin.denom == reward_denom.clone()
            || !exchange_rates
                .exchange_rates
                .iter()
                .any(|x| x.quote_denom == coin.denom)
        {
            continue;
        }

        messages.push(create_swap_msg(coin, reward_denom.to_string()));
    }

    let res = Response::new()
        .add_messages(messages)
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::LunaToPylunaHook {})?,
            funds: vec![],
        })])
        .add_attribute("action", "process_delegator_rewards");

    Ok(res)
}

pub fn luna_to_pyluna_hook(deps: DepsMut, env: Env) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    let reward_denom = String::from(REWARD_DENOM);

    let luna_amt = query_balance(
        &deps.querier,
        env.contract.address.clone(),
        reward_denom.clone(),
    )?;

    Ok(Response::new()
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.vault.to_string(),
                msg: to_binary(&VaultExecuteMsg::BondSplit { validator: None })?,
                funds: vec![Coin {
                    denom: reward_denom,
                    amount: luna_amt,
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::DepositMintedPylunaHook {})?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![attr("action", "luna_to_pyluna_hook")]))
}

pub fn deposit_minted_pyluna_hook(deps: DepsMut, env: Env) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    // query yluna amount to know how much we received from vault
    // received pluna amount should always be same as yluna amount
    let yluna_amt = query_token_balance(
        &deps.querier,
        cfg.yluna_token.clone(),
        env.contract.address.clone(),
    )?;
    let pluna_amt = query_token_balance(
        &deps.querier,
        cfg.pluna_token.clone(),
        env.contract.address.clone(),
    )?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::DepositRewards {
                assets: vec![
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: cfg.yluna_token,
                        },
                        amount: yluna_amt,
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: cfg.pluna_token,
                        },
                        amount: pluna_amt,
                    },
                ],
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![attr("action", "deposit_minted_pyluna_hook")]))
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
