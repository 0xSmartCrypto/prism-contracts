#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128, WasmMsg,
};

use crate::error::ContractError;
use crate::state::{Config, CONFIG};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};

use cw2::set_contract_version;
use cw_asset::{Asset, AssetInfo};
use prismswap::asset::{PrismSwapAsset, PrismSwapAssetInfo};
use prismswap::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};
use prismswap::querier::query_pair_info;

use std::collections::HashSet;

const CONTRACT_NAME: &str = "prism-collector";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        distribution_contract: deps.api.addr_validate(&msg.distribution_contract)?,
        astroport_factory: deps.api.addr_validate(&msg.astroport_factory)?,
        prismswap_factory: deps.api.addr_validate(&msg.prismswap_factory)?,
        prism_token: deps.api.addr_validate(&msg.prism_token)?,
        base_denom: msg.base_denom,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ConvertAndSend { assets, receiver } => {
            for asset in &assets {
                asset.info.check(deps.api)?;
            }
            convert_and_send(deps, env, info, assets, receiver)
        }
        ExecuteMsg::Distribute { asset_infos } => {
            for asset_info in &asset_infos {
                asset_info.check(deps.api)?;
            }
            distribute(deps, env, asset_infos)
        }
        ExecuteMsg::BaseSwapHook {
            receiver,
            prev_base_balance,
        } => base_swap_hook(deps, env, info, &receiver, prev_base_balance),
    }
}

pub fn convert_and_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
    receiver: Option<String>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut base_asset_input_funds = Uint128::zero();

    // issue TransferFrom calls for cw20, verify payment for native
    for asset in &assets {
        if asset.amount.is_zero() {
            continue;
        }

        match &asset.info {
            AssetInfo::Cw20(..) => {
                messages.push(
                    asset.transfer_from_msg(info.sender.clone(), env.contract.address.clone())?,
                );
            }
            AssetInfo::Native(denom) => {
                asset.assert_sent_native_token_balance(&info)?;
                if denom == &config.base_denom {
                    base_asset_input_funds = asset.amount;
                }
            }
        }
    }

    // validate reciever or set to sender if unset
    let receiver = match receiver {
        Some(addr_str) => deps.api.addr_validate(&addr_str)?,
        None => info.sender,
    };

    let (mut swap_msgs, need_hook) = get_swap_msgs(&deps, &env, &config, &assets, &receiver)?;

    // append prism conversion messages to any TransferFrom messages
    messages.append(&mut swap_msgs);

    // register base hook if needed.  we must set prev_base_balance to our
    // original base balance (current balance minus any funds sent with message).
    // This will prevent our original balance from being used inside the hook.
    if need_hook {
        let base_asset_info = AssetInfo::Native(config.base_denom);
        let original_base_balance = base_asset_info
            .query_balance(&deps.querier, env.contract.address.clone())?
            .checked_sub(base_asset_input_funds)
            .map_err(|e| StdError::Overflow { source: e })?;

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver,
                prev_base_balance: original_base_balance,
            })?,
            funds: vec![],
        }))
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "convert_and_send")]))
}

pub fn distribute(
    deps: DepsMut,
    env: Env,
    asset_infos: Vec<AssetInfo>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let mut assets: Vec<Asset> = vec![];

    // create asset objects for each assets_info using our current balance
    for asset_info in &asset_infos {
        let asset_balance =
            asset_info.query_balance(&deps.querier, env.contract.address.clone())?;

        assets.push(Asset {
            info: asset_info.clone(),
            amount: asset_balance,
        })
    }

    // receiver for this method is always the distribution contract
    let receiver = &config.distribution_contract;

    let (mut messages, need_hook) = get_swap_msgs(&deps, &env, &config, &assets, receiver)?;

    // register base hook if needed.  we set prev_base_balance to zero here which
    // allows the hook to consume the entire uusd contract balance, which is
    // desired here because any current uusd balance comes from protocol fees
    // and we want that swapped and sent directly to the distribution contract
    if need_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: receiver.clone(),
                prev_base_balance: Uint128::zero(),
            })?,
            funds: vec![],
        }))
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "distribute")]))
}

pub fn base_swap_hook(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receiver: &Addr,
    prev_base_balance: Uint128,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    // can only be called as a hook from this contract
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    // query contract balance for the base denom (uusd), exit if balance is zero
    let base_asset_info = AssetInfo::Native(config.base_denom.clone());
    let base_balance = base_asset_info.query_balance(&deps.querier, env.contract.address)?;
    let swap_amount = base_balance
        .checked_sub(prev_base_balance)
        .map_err(|e| StdError::Overflow { source: e })?;

    if swap_amount.is_zero() {
        return Ok(Response::new());
    }

    // create a base asset (uusd) object using our current balance minus
    // any balance passed in as prev_base_balance
    let base_asset = Asset {
        info: base_asset_info.clone(),
        amount: swap_amount,
    };

    // query prismswap for the uusd-prism pair, error on failure
    let prism_pair_addr =
        query_prismswap_prism_pair(&deps, &config, &base_asset_info).ok_or_else(|| {
            ContractError::MissingRoute {
                asset: base_asset.info.to_string(),
            }
        })?;

    // perform the final swap from uusd -> prism
    let swap_msg = get_swap_msg(&prism_pair_addr, &base_asset, receiver)?;

    Ok(Response::new()
        .add_message(swap_msg)
        .add_attribute("action", "base_swap_hook"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    config.as_res()
}

pub fn query_prismswap_prism_pair(
    deps: &DepsMut,
    config: &Config,
    asset_info: &AssetInfo,
) -> Option<Addr> {
    query_pair_info(
        &deps.querier,
        &config.prismswap_factory,
        &[
            asset_info.clone(),
            AssetInfo::Cw20(config.prism_token.clone()),
        ],
    )
    .ok()
    .map(|x| x.contract_addr)
}

pub fn query_prismswap_base_pair(
    deps: &DepsMut,
    config: &Config,
    asset_info: &AssetInfo,
) -> Option<Addr> {
    query_pair_info(
        &deps.querier,
        &config.prismswap_factory,
        &[
            AssetInfo::Native(config.base_denom.clone()),
            asset_info.clone(),
        ],
    )
    .ok()
    .map(|x| x.contract_addr)
}

pub fn query_astroport_base_pair(
    deps: &DepsMut,
    config: &Config,
    asset_info: &AssetInfo,
) -> Option<Addr> {
    astroport::querier::query_pair_info(
        &deps.querier,
        config.astroport_factory.clone(),
        &[
            AssetInfo::Native(config.base_denom.clone()).into(),
            asset_info.into(),
        ],
    )
    .ok()
    .map(|x| x.contract_addr)
}

pub fn get_swap_msgs(
    deps: &DepsMut,
    env: &Env,
    config: &Config,
    assets: &[Asset],
    receiver: &Addr,
) -> Result<(Vec<CosmosMsg>, bool), ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];
    let mut need_hook = false;
    let mut base_swap_asset = None;

    let base_asset_info = AssetInfo::Native(config.base_denom.clone());
    let prism_asset_info = AssetInfo::Cw20(config.prism_token.clone());

    let asset_set: HashSet<String> = assets.iter().map(|asset| asset.info.to_string()).collect();
    if asset_set.len() != assets.len() {
        return Err(ContractError::DuplicateAssets {});
    }

    for asset in assets {
        if asset.amount.is_zero() {
            continue;
        }
        // check for base and prism asset, which contain specialized logic.
        // for the base asset, we make a delayed decision based on whether or
        // not we need the hook for any other asset.  If so, we don't need
        // to do anything and we'll convert the uusd as part of the hook.  If not,
        // then we do a direct swap from uusd to prism at the end of this method.
        // for the prism asset, we always transfer immediately to receiver.
        if asset.info == base_asset_info {
            base_swap_asset = Some(asset);
            continue;
        } else if asset.info == prism_asset_info {
            let transfer_msg = asset.transfer_msg(receiver.clone())?;
            messages.push(transfer_msg);
            continue;
        }

        // try to query pair with $PRISM
        let prism_pair_addr = query_prismswap_prism_pair(deps, config, &asset.info);

        if let Some(pair_addr) = prism_pair_addr {
            // direct pair exists from asset -> PRISM
            let swap_msg = get_swap_msg(&pair_addr, asset, receiver)?;
            messages.push(swap_msg);
        } else {
            // check for an indirect route from asset -> uusd, error if not found
            let base_pair_addr = query_prismswap_base_pair(deps, config, &asset.info)
                .or_else(|| query_astroport_base_pair(deps, config, &asset.info))
                .ok_or_else(|| ContractError::MissingRoute {
                    asset: asset.info.to_string(),
                })?;

            // for indirect route, swap receiver should be set to our contract,
            // it will get sent to receiver inside the BaseSwapHook message
            let swap_msg = get_swap_msg(&base_pair_addr, asset, &env.contract.address)?;
            messages.push(swap_msg);

            // requires hook to perform final uusd -> PRISM conversion
            need_hook = true;
        }
    }

    // if there's no need for the hook, but we still want to convert some
    // uusd to prism, then we'll do that here and send directly to receiver
    if !need_hook {
        if let Some(swap_asset) = base_swap_asset {
            let pair_addr =
                query_prismswap_prism_pair(deps, config, &swap_asset.info).ok_or_else(|| {
                    ContractError::MissingRoute {
                        asset: swap_asset.info.to_string(),
                    }
                })?;
            let swap_msg = get_swap_msg(&pair_addr, swap_asset, receiver)?;
            messages.push(swap_msg);
        }
    }

    Ok((messages, need_hook))
}

pub fn get_swap_msg(
    pair_addr: &Addr,
    offer_asset: &Asset,
    recipient: &Addr,
) -> StdResult<CosmosMsg> {
    match &offer_asset.info {
        AssetInfo::Cw20(..) => {
            let msg = to_binary(&PairCw20HookMsg::Swap {
                max_spread: None,
                belief_price: None,
                to: Some(recipient.to_string()),
            })?;
            offer_asset.send_msg(pair_addr, msg)
        }
        AssetInfo::Native(denom) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_addr.to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: offer_asset.clone(),
                max_spread: None,
                belief_price: None,
                to: Some(recipient.to_string()),
            })?,
            funds: vec![Coin {
                denom: denom.to_string(),
                amount: offer_asset.amount,
            }],
        })),
    }
}
