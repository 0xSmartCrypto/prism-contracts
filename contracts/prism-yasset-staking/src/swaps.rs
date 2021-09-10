use crate::state::{CONFIG, TOTAL_BOND_AMOUNT};
use cosmwasm_std::{
    attr, to_binary, Addr, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::yasset_staking::ExecuteMsg;
use std::cmp::min;
use terra_cosmwasm::{create_swap_msg, ExchangeRatesResponse, TerraMsgWrapper, TerraQuerier};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::ExecuteMsg as TerraswapExecuteMsg;
use terraswap::querier::{query_balance, query_token_balance};

/// Swap all native tokens to reward_denom
/// Only hub_contract is allowed to execute
pub fn swap_to_reward_denom(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != cfg.vault {
        return Err(StdError::generic_err("unauthorized"));
    }

    let contr_addr = env.contract.address;
    let balance = deps.querier.query_all_balances(contr_addr)?;
    let mut messages: Vec<SubMsg<TerraMsgWrapper>> = Vec::new();

    let reward_denom = cfg.reward_denom;

    let mut is_listed = true;

    let denoms: Vec<String> = balance.iter().map(|item| item.denom.clone()).collect();

    if query_exchange_rates(&deps, reward_denom.clone(), denoms).is_err() {
        is_listed = false;
    }

    for coin in balance {
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
        .add_attributes(vec![attr("action", "swap")]);

    Ok(res)
}

pub fn swap_to_prism(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != cfg.vault {
        return Err(StdError::generic_err("unauthorized"));
    }

    let prism_amt = query_token_balance(
        &deps.querier,
        Addr::unchecked(cfg.prism_token),
        env.contract.address.clone(),
    )?;

    let offer_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: cfg.reward_denom.clone(),
        },
        amount: query_balance(
            &deps.querier,
            env.contract.address.clone(),
            cfg.reward_denom.clone(),
        )?,
    };

    let amount = (offer_asset.deduct_tax(&deps.querier)?).amount;

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.prism_pair,
            msg: to_binary(&TerraswapExecuteMsg::Swap {
                offer_asset: Asset {
                    amount,
                    ..offer_asset
                },
                belief_price: None,
                max_spread: None,
                to: None,
            })?,
            funds: vec![Coin {
                denom: cfg.reward_denom.clone(),
                amount,
            }],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::DepositPrism {
                old_amount: prism_amt,
            })?,
            funds: vec![],
        }),
    ]))
}

pub fn deposit_prism(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    old_amount: Uint128,
) -> StdResult<Response<TerraMsgWrapper>> {
    if info.sender.as_str() != env.contract.address {
        return Err(StdError::generic_err("unauthorized"));
    }

    let cfg = CONFIG.load(deps.storage)?;

    let prism_amt = query_token_balance(
        &deps.querier,
        Addr::unchecked(cfg.prism_token.clone()),
        env.contract.address.clone(),
    )?;

    let total_to_deposit = prism_amt - old_amount;

    let total_luna = query_balance(
        &deps.querier,
        Addr::unchecked(cfg.vault),
        "uluna".to_owned(),
    )?;

    let yluna_staked = TOTAL_BOND_AMOUNT.load(deps.storage)?;

    // if all yluna has been staked, and there has recently been a slashing event
    // its possible yluna_staked / total_luna > 1, hence why min needed
    let for_stakers = min(
        total_to_deposit,
        total_to_deposit
            .multiply_ratio(yluna_staked, total_luna)
            .multiply_ratio(9u128, 10u128),
    );

    let to_deposit_stakers = Asset {
        info: AssetInfo::Token {
            contract_addr: cfg.prism_token.clone(),
        },
        amount: for_stakers,
    };

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::DepositRewards {
                assets: vec![to_deposit_stakers],
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: cfg.gov,
                amount: total_to_deposit - for_stakers,
            })?,
            funds: vec![],
        }),
    ]))
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
