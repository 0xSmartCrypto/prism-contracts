use crate::state::CONFIG;
use cosmwasm_std::{
    attr, to_binary, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, WasmMsg,
};
use cw_asset::{Asset, AssetInfo};
use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::yasset_staking::ExecuteMsg;
use prismswap::querier::{query_balance, query_token_balance};
use terra_cosmwasm::{create_swap_msg, ExchangeRatesResponse, TerraMsgWrapper, TerraQuerier};

pub const REWARD_DENOM: &str = "uluna";

/// 1. Swap all native tokens to uluna
/// 2. Use the uluna to mint pluna and yluna
/// 3. Deposit pluna and yluna as reward to stakers
///
/// This method should be called after native delegator rewards have already
/// been deposited into this contract.
pub fn process_delegator_rewards(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> StdResult<Response<TerraMsgWrapper>> {
    // Find all native denoms for which we have a balance.
    let balances = deps.querier.query_all_balances(&env.contract.address)?;
    let denoms: Vec<String> = balances.iter().map(|item| item.denom.clone()).collect();

    let reward_denom = String::from(REWARD_DENOM);
    let exchange_rates = query_exchange_rates(&deps, reward_denom.clone(), denoms)?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = Vec::new();
    for coin in balances {
        if coin.denom == reward_denom
            || !exchange_rates
                .exchange_rates
                .iter()
                .any(|x| x.quote_denom == coin.denom)
        {
            // ignore luna and any other denom that's not convertible to luna.
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

    let luna_amt = query_balance(&deps.querier, &env.contract.address, reward_denom.clone())?;

    // Record the current balance to know how much was minted when
    // DepositMintedPylunaHook is executed right after.
    let prev_pluna_balance =
        query_token_balance(&deps.querier, &cfg.pluna_token, &env.contract.address)?;
    let prev_yluna_balance =
        query_token_balance(&deps.querier, &cfg.yluna_token, &env.contract.address)?;

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
                msg: to_binary(&ExecuteMsg::DepositMintedPylunaHook {
                    prev_pluna_balance,
                    prev_yluna_balance,
                })?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![attr("action", "luna_to_pyluna_hook")]))
}

pub fn deposit_minted_pyluna_hook(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    prev_pluna_balance: Uint128,
    prev_yluna_balance: Uint128,
) -> StdResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender != env.contract.address {
        return Err(StdError::generic_err("unauthorized"));
    }

    // query pluna amount to know how much we received from vault
    // received pluna amount should always be same as yluna amount
    // we query both amounts to prevent manipulation by sending one of the tokens to the contract
    let curr_pluna_balance =
        query_token_balance(&deps.querier, &cfg.pluna_token, &env.contract.address)?;
    let curr_yluna_balance =
        query_token_balance(&deps.querier, &cfg.yluna_token, &env.contract.address)?;

    let reward_pluna = curr_pluna_balance.checked_sub(prev_pluna_balance)?;
    let reward_yluna = curr_yluna_balance.checked_sub(prev_yluna_balance)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::DepositRewards {
                assets: vec![
                    Asset {
                        info: AssetInfo::Cw20(cfg.yluna_token),
                        amount: reward_yluna,
                    },
                    Asset {
                        info: AssetInfo::Cw20(cfg.pluna_token),
                        amount: reward_pluna,
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
