#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use std::str::FromStr;

use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, WasmMsg,
};

use crate::error::ContractError;
use crate::migration::migrate_config;
use crate::state::{Config, CONFIG};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

use astroport::pair::{Cw20HookMsg as AstroPairCw20HookMsg, ExecuteMsg as AstroPairExecuteMsg};
use cw2::set_contract_version;
use cw_asset::{Asset, AssetInfo};
use prism_common::permissions::check_sender;
use prismswap::asset::{PrismSwapAsset, PrismSwapAssetInfo};
use prismswap::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};
use prismswap::querier::query_pair_info;
use prismswap::router::{
    Cw20HookMsg as RouterCw20HookMsg, ExecuteMsg as RouterExecuteMsg, SwapOperation,
};

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
        prismswap_router: deps.api.addr_validate(&msg.prismswap_router)?,
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
        // Public endpoints (wide open to the entire internet).
        ExecuteMsg::ConvertAndSend {
            assets,
            receiver,
            dest_asset_info,
        } => {
            for asset in &assets {
                asset.info.check(deps.api)?;
            }
            dest_asset_info.check(deps.api)?;
            convert_and_send(deps, env, info, assets, receiver, dest_asset_info)
        }
        ExecuteMsg::Distribute { asset_infos } => {
            for asset_info in &asset_infos {
                asset_info.check(deps.api)?;
            }
            distribute(deps, env, asset_infos)
        }
        _ => {
            // Private endpoints (open to specific callers only).
            match msg {
                ExecuteMsg::BaseSwapHook {
                    receiver,
                    prev_base_balance,
                    dest_asset_info,
                } => {
                    // Can only be self-called as a hook from this contract.
                    check_sender(&info, &env.contract.address)?;
                    base_swap_hook(
                        deps,
                        env,
                        info,
                        &receiver,
                        prev_base_balance,
                        dest_asset_info,
                    )
                }
                _ => Err(ContractError::NotImplemented {}),
            }
        }
    }
}

pub fn convert_and_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
    receiver: Option<String>,
    dest_asset_info: AssetInfo,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut base_asset_input_amt = Uint128::zero();

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
                    base_asset_input_amt = asset.amount;
                }
            }
        }
    }

    // Validate receiver or set to sender if unset
    let receiver = match receiver {
        Some(addr_str) => deps.api.addr_validate(&addr_str)?,
        None => info.sender,
    };

    // get all the messages required to perform the swap, this also returns
    // whether we need to register for the base hook
    let (mut swap_msgs, need_hook) =
        get_swap_msgs(&deps, &env, &config, &assets, &receiver, &dest_asset_info)?;

    // append swap messages to any TransferFrom messages
    messages.append(&mut swap_msgs);

    // register base hook if needed.  we must set prev_base_balance to our
    // original base balance (current balance minus any funds sent with message).
    // This will prevent our original balance from being used inside the hook.
    if need_hook {
        let base_asset_info = AssetInfo::Native(config.base_denom);
        let original_base_balance = base_asset_info
            .query_balance(&deps.querier, env.contract.address.clone())?
            .checked_sub(base_asset_input_amt)
            .map_err(|e| StdError::Overflow { source: e })?;

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver,
                prev_base_balance: original_base_balance,
                dest_asset_info,
            })?,
            funds: vec![],
        }));
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

    // desination for this method is always prism
    let dest_asset_info = AssetInfo::Cw20(config.prism_token.clone());

    let (mut messages, need_hook) =
        get_swap_msgs(&deps, &env, &config, &assets, receiver, &dest_asset_info)?;

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
                dest_asset_info,
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
    dest_asset_info: AssetInfo,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    // query contract balance for the base denom (uusd), exit if balance is zero
    let base_asset_info = AssetInfo::Native(config.base_denom.clone());
    let base_balance =
        base_asset_info.query_balance(&deps.querier, env.contract.address.clone())?;
    let swap_amount = base_balance
        .checked_sub(prev_base_balance)
        .map_err(|e| StdError::Overflow { source: e })?;

    if swap_amount.is_zero() {
        return Ok(Response::new());
    }

    // create a base asset (uusd) object using our current balance minus
    // any balance passed in as prev_base_balance
    let base_asset = Asset {
        info: base_asset_info,
        amount: swap_amount,
    };

    // get the final swap messages
    let (swap_msgs, need_hook) = get_swap_msgs(
        &deps,
        &env,
        &config,
        &[base_asset],
        receiver,
        &dest_asset_info,
    )?;

    // need_hook should never be set here, logic error if it is
    if need_hook {
        return Err(ContractError::LogicError {
            msg: "need_hook set inside base_swap_hook, should not happen".to_string(),
        });
    };

    Ok(Response::new()
        .add_messages(swap_msgs)
        .add_attribute("action", "base_swap_hook"))
}

pub fn get_swap_msgs(
    deps: &DepsMut,
    env: &Env,
    cfg: &Config,
    assets: &[Asset],
    receiver: &Addr,
    dest_asset_info: &AssetInfo,
) -> Result<(Vec<CosmosMsg>, bool), ContractError> {
    let mut msgs = vec![];
    let mut need_hook = false;

    // check for duplicates inpu assets, not allowed
    let asset_set: HashSet<String> = assets.iter().map(|asset| asset.info.to_string()).collect();
    if asset_set.len() != assets.len() {
        return Err(ContractError::DuplicateAssets {});
    }

    for asset in assets {
        if asset.amount.is_zero() {
            continue;
        }

        // check for dest asset, transfer directly to receiver.
        if &asset.info == dest_asset_info {
            let transfer_msg = asset.transfer_msg(receiver.clone())?;
            msgs.push(transfer_msg);
            continue;
        }

        let route = get_swap_route(deps, cfg, &asset.info, dest_asset_info);
        let swap_msg = match route {
            Some(SwapRoute::PrismSwapDirect(pair_addr)) => {
                get_prism_direct_swap_msg(&pair_addr, asset, receiver)?
            }
            Some(SwapRoute::PrismSwapRouter(..)) => {
                let base_asset_info = AssetInfo::Cw20(cfg.prism_token.clone());
                get_prism_router_swap_msg(cfg, asset, dest_asset_info, &base_asset_info, receiver)?
            }
            Some(SwapRoute::AstroportToBase(pair_addr)) => {
                need_hook = true;
                get_astro_direct_swap_msg(&pair_addr, asset, &env.contract.address)?
            }
            None => {
                return Err(ContractError::MissingRoute {
                    asset: asset.info.clone(),
                    dest_asset: dest_asset_info.clone(),
                });
            }
        };
        msgs.push(swap_msg);
    }
    Ok((msgs, need_hook))
}

#[derive(Clone, Debug, PartialEq)]
pub enum SwapRoute {
    PrismSwapDirect(Addr),
    PrismSwapRouter(Addr, Addr),
    AstroportToBase(Addr),
}

pub fn get_swap_route(
    deps: &DepsMut,
    cfg: &Config,
    offer_asset_info: &AssetInfo,
    dest_asset_info: &AssetInfo,
) -> Option<SwapRoute> {
    // check for prismswap direct route
    let prismswap_direct_asset_infos = [offer_asset_info.clone(), dest_asset_info.clone()];
    if let Some(pair_addr) = query_prismswap_pair(deps, cfg, &prismswap_direct_asset_infos) {
        return Some(SwapRoute::PrismSwapDirect(pair_addr));
    } else {
        // check for prismswap 3-way router swap using prism as intermediate hop
        // e.g. offer -> prism, prism -> dest
        let prism_asset_info = AssetInfo::Cw20(cfg.prism_token.clone());
        let swap1_asset_infos = [offer_asset_info.clone(), prism_asset_info.clone()];
        if let Some(pair1_addr) = query_prismswap_pair(deps, cfg, &swap1_asset_infos) {
            let swap2_asset_infos = [dest_asset_info.clone(), prism_asset_info];
            if let Some(pair2_addr) = query_prismswap_pair(deps, cfg, &swap2_asset_infos) {
                return Some(SwapRoute::PrismSwapRouter(pair1_addr, pair2_addr));
            }
        } else {
            // check for astroport offer -> base
            let astroport_direct_asset_infos = [
                offer_asset_info.clone(),
                AssetInfo::Native(cfg.base_denom.clone()),
            ];
            let astro_pair = query_astroport_pair(deps, cfg, &astroport_direct_asset_infos);
            if let Some(pair_addr) = astro_pair {
                return Some(SwapRoute::AstroportToBase(pair_addr));
            }
        }
    }
    None
}

pub fn query_prismswap_pair(
    deps: &DepsMut,
    config: &Config,
    asset_infos: &[AssetInfo; 2],
) -> Option<Addr> {
    query_pair_info(&deps.querier, &config.prismswap_factory, asset_infos)
        .ok()
        .map(|x| x.contract_addr)
}

pub fn query_astroport_pair(
    deps: &DepsMut,
    config: &Config,
    asset_infos: &[AssetInfo; 2],
) -> Option<Addr> {
    let astro_asset_infos: [astroport::asset::AssetInfo; 2] =
        [asset_infos[0].clone().into(), asset_infos[1].clone().into()];

    astroport::querier::query_pair_info(
        &deps.querier,
        config.astroport_factory.clone(),
        &astro_asset_infos,
    )
    .ok()
    .map(|x| x.contract_addr)
}

pub fn get_prism_direct_swap_msg(
    pair_addr: &Addr,
    offer_asset: &Asset,
    receiver: &Addr,
) -> Result<CosmosMsg, ContractError> {
    match &offer_asset.info {
        AssetInfo::Cw20(..) => {
            let msg = PairCw20HookMsg::Swap {
                max_spread: None,
                belief_price: None,
                to: Some(receiver.to_string()),
            };
            offer_asset
                .send_msg(pair_addr, to_binary(&msg)?)
                .map_err(ContractError::Std)
        }
        AssetInfo::Native(..) => {
            let msg = PairExecuteMsg::Swap {
                offer_asset: offer_asset.clone(),
                max_spread: None,
                belief_price: None,
                to: Some(receiver.to_string()),
            };
            send_msg_with_native_funds(offer_asset, pair_addr, to_binary(&msg)?)
        }
    }
}

pub fn get_prism_router_swap_msg(
    cfg: &Config,
    offer_asset: &Asset,
    ask_asset_info: &AssetInfo,
    base_asset_info: &AssetInfo,
    receiver: &Addr,
) -> Result<CosmosMsg, ContractError> {
    if let AssetInfo::Cw20(..) = offer_asset.info {
        let msg = RouterCw20HookMsg::ExecuteSwapOperations {
            operations: vec![
                SwapOperation::PrismSwap {
                    offer_asset_info: offer_asset.info.clone(),
                    ask_asset_info: base_asset_info.clone(),
                },
                SwapOperation::PrismSwap {
                    offer_asset_info: base_asset_info.clone(),
                    ask_asset_info: ask_asset_info.clone(),
                },
            ],
            minimum_receive: None,
            to: Some(receiver.clone()),
        };
        offer_asset
            .send_msg(cfg.prismswap_router.clone(), to_binary(&msg)?)
            .map_err(ContractError::Std)
    } else {
        let msg = RouterExecuteMsg::ExecuteSwapOperations {
            operations: vec![
                SwapOperation::PrismSwap {
                    offer_asset_info: offer_asset.info.clone(),
                    ask_asset_info: base_asset_info.clone(),
                },
                SwapOperation::PrismSwap {
                    offer_asset_info: base_asset_info.clone(),
                    ask_asset_info: ask_asset_info.clone(),
                },
            ],
            minimum_receive: None,
            to: Some(receiver.clone()),
        };
        send_msg_with_native_funds(offer_asset, &cfg.prismswap_router, to_binary(&msg)?)
    }
}

pub fn get_astro_direct_swap_msg(
    pair_addr: &Addr,
    offer_asset: &Asset,
    receiver: &Addr,
) -> Result<CosmosMsg, ContractError> {
    let max_spread = Decimal::from_str(astroport::pair::MAX_ALLOWED_SLIPPAGE)?;

    match &offer_asset.info {
        AssetInfo::Cw20(..) => {
            let msg = AstroPairCw20HookMsg::Swap {
                max_spread: Some(max_spread),
                belief_price: None,
                to: Some(receiver.to_string()),
            };
            offer_asset
                .send_msg(pair_addr, to_binary(&msg)?)
                .map_err(ContractError::Std)
        }
        AssetInfo::Native(..) => {
            let msg = AstroPairExecuteMsg::Swap {
                offer_asset: offer_asset.into(),
                max_spread: Some(max_spread),
                belief_price: None,
                to: Some(receiver.to_string()),
            };
            send_msg_with_native_funds(offer_asset, pair_addr, to_binary(&msg)?)
        }
    }
}

pub fn send_msg_with_native_funds(
    asset: &Asset,
    contract_addr: &Addr,
    msg: Binary,
) -> Result<CosmosMsg, ContractError> {
    if let AssetInfo::Native(denom) = &asset.info {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg,
            funds: vec![Coin {
                denom: denom.to_string(),
                amount: asset.amount,
            }],
        }))
    } else {
        return Err(ContractError::LogicError {
            msg: format!(
                "send_msg_with_native_funds on non-native asset: {}",
                asset.info
            ),
        });
    }
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    let router: Addr = deps.api.addr_validate(&msg.prismswap_router)?;
    migrate_config(deps.storage, router)?;

    Ok(Response::default())
}
