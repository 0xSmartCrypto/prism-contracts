#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, WasmMsg,
};

use prism_protocol::reward_distribution::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, RewardAssetWhitelistResponse,
};

use prism_protocol::yasset_staking::ExecuteMsg as StakingExecuteMsg;

use crate::error::{ContractError, ContractResult};
use crate::querier::{
    query_vault_bond_amount, query_yasset_staking_bond_amount, query_yasset_staking_x_bond_amount,
};
use crate::state::{Config, CONFIG, WHITELISTED_ASSETS};
use cw_asset::{Asset, AssetInfo};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use terra_cosmwasm::{create_swap_msg, ExchangeRatesResponse, TerraMsgWrapper, TerraQuerier};

const CONTRACT_NAME: &str = "prism-reward-distribution";
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

    if msg.protocol_fee > Decimal::one() {
        return Err(ContractError::InvalidConfig {
            reason: "invalid protocol fee".to_string(),
        });
    }

    if msg.whitelisted_assets.is_empty() {
        return Err(ContractError::InvalidConfig {
            reason: "whitelisted assets cannot be empty".to_string(),
        });
    }

    CONFIG.save(
        deps.storage,
        &Config {
            vault: deps.api.addr_validate(&msg.vault)?,
            gov: deps.api.addr_validate(&msg.gov)?,
            yasset_token: deps.api.addr_validate(&msg.yasset_token)?,
            yasset_staking: deps.api.addr_validate(&msg.yasset_staking)?,
            yasset_staking_x: deps.api.addr_validate(&msg.yasset_staking_x)?,
            collector: deps.api.addr_validate(&msg.collector)?,
            protocol_fee: msg.protocol_fee,
        },
    )?;

    WHITELISTED_ASSETS.save(deps.storage, &msg.whitelisted_assets)?;

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
        ExecuteMsg::DistributeRewards { asset_infos } => {
            distribute_rewards(deps, env, info, asset_infos)
        }
        ExecuteMsg::WhitelistRewardAsset { asset } => whitelist_reward_asset(deps, info, asset),
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
            msg: to_binary(&ExecuteMsg::DistributeRewards {
                asset_infos: vec![AssetInfo::Native(reward_denom)],
            })?,
            funds: vec![],
        })])
        .add_attribute("action", "distribute");

    Ok(res)
}

pub fn distribute_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset_infos: Vec<AssetInfo>,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.vault {
        return Err(ContractError::Unauthorized {});
    }

    let vault_bond_amount = query_vault_bond_amount(&deps.querier, cfg.vault)?;
    let yasset_staking_bonded =
        query_yasset_staking_bond_amount(&deps.querier, cfg.yasset_staking.clone())?;
    let yasset_staking_x_bonded =
        query_yasset_staking_x_bond_amount(&deps.querier, cfg.yasset_staking_x.clone())?;
    let whitelisted_assets = query_whitelist(deps.as_ref())?.assets;
    let total_bond_amount = yasset_staking_bonded + yasset_staking_x_bonded;

    if vault_bond_amount.is_zero() {
        return Err(ContractError::EmptyVault {});
    }

    let mut messages = vec![];
    let stakers_portion = Decimal::from_ratio(total_bond_amount, vault_bond_amount);
    for asset_info in asset_infos {
        if !whitelisted_assets.contains(&asset_info) {
            return Err(ContractError::RewardAssetNotWhitelisted {
                asset: asset_info.to_string(),
            });
        }
        
        let balance = asset_info.query_balance(&deps.querier, env.contract.address.clone())?; 
        let stakers_portion_amount = balance * stakers_portion;
        let protocol_fee_amount = stakers_portion_amount * cfg.protocol_fee;
        let reward_amount = stakers_portion_amount
            .checked_sub(protocol_fee_amount)
            .map_err(|x| -> StdError { x.into() })?;

        let collector_asset = Asset {
            info: asset_info.clone(),
            amount: balance
                .checked_sub(reward_amount)
                .map_err(|x| -> StdError { x.into() })?,
        };

        // send the collector portion
        messages.push(get_transfer_asset_msg(
            collector_asset,
            cfg.collector.clone(),
        )?);

        // send the staker portion, pro-rata split between yasset-staking and yasset-staking-x
        if !total_bond_amount.is_zero() {
            let yasset_staking_asset = Asset {
                info: asset_info.clone(),
                amount: reward_amount
                    * Decimal::from_ratio(yasset_staking_bonded, total_bond_amount),
            };
            let yasset_staking_x_asset = Asset {
                info: asset_info,
                amount: reward_amount - yasset_staking_asset.amount,
            };

            if !yasset_staking_asset.amount.is_zero() {
                let mut msgs =
                    get_deposit_rewards_msgs(yasset_staking_asset, cfg.yasset_staking.clone())?;
                messages.append(&mut msgs);
            }

            if !yasset_staking_x_asset.amount.is_zero() {
                let mut msgs =
                    get_deposit_rewards_msgs(yasset_staking_x_asset, cfg.yasset_staking_x.clone())?;
                messages.append(&mut msgs);
            }
        }
    }
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "distribute_rewards"))
}

pub fn whitelist_reward_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;

    // can only be exeucted by gov
    if info.sender.as_str() != cfg.gov.as_str() {
        return Err(ContractError::Unauthorized {});
    }

    let mut whitelist = WHITELISTED_ASSETS.load(deps.storage)?;
    whitelist.push(asset.clone());

    WHITELISTED_ASSETS.save(deps.storage, &whitelist)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "whitelist_reward_asset"),
        attr("whitelisted_asset", asset.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::RewardAssetWhitelist {} => to_binary(&query_whitelist(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        vault: cfg.vault.to_string(),
        gov: cfg.gov.to_string(),
        yasset_token: cfg.yasset_token.to_string(),
        yasset_staking: cfg.yasset_staking.to_string(),
        yasset_staking_x: cfg.yasset_staking_x.to_string(),
        collector: cfg.collector.to_string(),
        protocol_fee: cfg.protocol_fee,
    })
}

pub fn query_whitelist(deps: Deps) -> StdResult<RewardAssetWhitelistResponse> {
    let whitelist = WHITELISTED_ASSETS.load(deps.storage)?;
    Ok(RewardAssetWhitelistResponse { assets: whitelist })
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

pub fn get_transfer_asset_msg(
    asset: Asset,
    recipient: Addr,
) -> StdResult<CosmosMsg<TerraMsgWrapper>> {
    match &asset.info {
        AssetInfo::Cw20(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient.to_string(),
                amount: asset.amount,
            })?,
            funds: vec![],
        })),
        AssetInfo::Native(denom) => Ok(CosmosMsg::Bank(BankMsg::Send {
            to_address: recipient.to_string(),
            amount: vec![Coin {
                denom: denom.to_string(),
                amount: asset.amount
            }],
        })),
    }
}

pub fn get_deposit_rewards_msgs(
    asset: Asset,
    staking_addr: Addr,
) -> ContractResult<Vec<CosmosMsg<TerraMsgWrapper>>> {
    match asset.info.clone() {
        AssetInfo::Native(denom) => Ok(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: staking_addr.to_string(),
            msg: to_binary(&StakingExecuteMsg::DepositRewards {
                assets: vec![asset.clone()],
            })?,
            funds: vec![Coin {
                denom,
                amount: asset.amount,
            }],
        })]),
        AssetInfo::Cw20(contract_addr) => Ok(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: staking_addr.to_string(),
                    amount: asset.amount,
                    expires: None,
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: staking_addr.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![asset],
                })?,
                funds: vec![],
            }),
        ]),
    }
}
