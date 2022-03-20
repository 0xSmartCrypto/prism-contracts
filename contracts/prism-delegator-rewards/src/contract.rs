#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, WasmMsg,
};


use crate::error::{ContractError, ContractResult};
use crate::state::{Config, CONFIG};
use cw_asset::{Asset, AssetInfo};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use terra_cosmwasm::{create_swap_msg, ExchangeRatesResponse, TerraMsgWrapper, TerraQuerier};
use prismswap::querier::{query_balance};

use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::reward_distribution::ExecuteMsg as RewardDistributionExecuteMsg;
use prism_protocol::delegator_rewards::{ExecuteMsg, QueryMsg, InstantiateMsg, ConfigResponse};

const CONTRACT_NAME: &str = "prism-delegator-rewards";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const REWARD_DENOM: &str = "uluna";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONFIG.save(
        deps.storage,
        &Config {
            owner: deps.api.addr_validate(&msg.owner)?,
            vault: deps.api.addr_validate(&msg.vault)?,
            yluna_token: deps.api.addr_validate(&msg.yluna_token)?,
            pluna_token: deps.api.addr_validate(&msg.pluna_token)?,
            reward_distribution: deps.api.addr_validate(&msg.reward_distribution)?,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response<TerraMsgWrapper>> {
    match msg {
        ExecuteMsg::ProcessDelegatorRewards {} => process_delegator_rewards(deps, env, info),
        ExecuteMsg::LunaToPylunaHook {} => luna_to_pyluna_hook(deps, info, env),
        ExecuteMsg::DistributeMintedPylunaHook {
        } => distribute_minted_pyluna_hook(deps, info, env),
        ExecuteMsg::UpdateConfig {
            owner
        } => update_config(deps, info, owner),
    }
}

pub fn process_delegator_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.vault {
        return Err(ContractError::Unauthorized {});
    }

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

/// 1. Use the uluna to mint pluna and yluna
/// 2. Deposit pluna and yluna as reward to stakers
pub fn luna_to_pyluna_hook(
    deps: DepsMut, 
    info: MessageInfo,
    env: Env
) -> ContractResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized{});
    }

    let reward_denom = String::from(REWARD_DENOM);

    let luna_amt = query_balance(&deps.querier, &env.contract.address, reward_denom.clone())?;

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
                msg: to_binary(&ExecuteMsg::DistributeMintedPylunaHook {})?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![attr("action", "luna_to_pyluna_hook")]))
}

pub fn distribute_minted_pyluna_hook(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized{});
    }

    let pluna_asset_info = AssetInfo::Cw20(cfg.pluna_token.clone());
    let pluna_asset = Asset {
        info: pluna_asset_info.clone(),
        amount: pluna_asset_info.query_balance(&deps.querier, env.contract.address.clone())?,
    };

    let yluna_asset_info = AssetInfo::Cw20(cfg.yluna_token.clone());
    let yluna_asset = Asset {
        info: yluna_asset_info.clone(),
        amount: yluna_asset_info.query_balance(&deps.querier, env.contract.address.clone())?,
    };

    Ok(Response::new()
        .add_messages(
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: cfg.pluna_token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: cfg.reward_distribution.to_string(),
                        amount: pluna_asset.amount,
                    })?,
                    funds: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: cfg.yluna_token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: cfg.reward_distribution.to_string(),
                        amount: yluna_asset.amount,
                    })?,
                    funds: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: cfg.reward_distribution.to_string(),
                    msg: to_binary(&RewardDistributionExecuteMsg::DistributeRewards {})?,
                    funds: vec![],
                })
            ])
        .add_attributes(vec![attr("action", "distribute_minted_pyluna_hook")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        vault: cfg.vault.to_string(),
        yluna_token: cfg.yluna_token.to_string(),
        pluna_token: cfg.pluna_token.to_string(),
        reward_distribution: cfg.reward_distribution.to_string(),
    })
}


fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by owner
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized{});
    }

    if let Some(owner) = owner {
        cfg.owner = deps.api.addr_validate(&owner)?;
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
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
