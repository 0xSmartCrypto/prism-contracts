#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128, WasmMsg,
};

use crate::state::{Config, CONFIG};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};

use cw2::set_contract_version;
use cw_asset::{Asset, AssetInfo, AssetInfoUnchecked, AssetUnchecked};
use prismswap::asset::{Asset as PSAsset, AssetInfo as PSAssetInfo};
use prismswap::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};
use prismswap::querier::query_pair_info;

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
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::ConvertAndSend { assets, receiver } => {
            convert_and_send(deps, env, info, assets, receiver)
        }
        ExecuteMsg::Distribute { asset_infos } => distribute(deps, env, asset_infos),
        ExecuteMsg::BaseSwapHook { receiver } => base_swap_hook(deps, env, info, receiver),
    }
}

pub fn verify_payment(info: &MessageInfo, denom: &str, amount: Uint128) -> StdResult<()> {
    let coin_payment = info
        .funds
        .iter()
        .find(|x| x.denom == denom && x.amount > Uint128::zero())
        .ok_or_else(|| StdError::generic_err(format!("Missing funds payment: {}", denom)))?;
    if coin_payment.amount != amount {
        return Err(StdError::generic_err(format!(
            "Invalid {} payment - funds/asset amount mismatch",
            denom
        )));
    }
    Ok(())
}

pub fn convert_and_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<PSAsset>,
    receiver: Option<String>,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    let receiver = match receiver {
        None => info.sender.clone(),
        Some(addr_str) => deps.api.addr_validate(&addr_str)?,
    };

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut need_hook: bool = false;
    for asset in assets {
        //let asset: Asset = asset.into::<AssetUnchecked>().check(deps.api)?;
        let asset: Asset = AssetUnchecked::from(asset).check(deps.api)?;
        match &asset.info {
            AssetInfo::Cw20(..) => {
                messages.push(
                    asset.transfer_from_msg(info.sender.clone(), env.contract.address.clone())?,
                );
            }
            AssetInfo::Native(denom) => {
                verify_payment(&info, denom, asset.amount)?;
            }
        }

        // try to query pair with $PRISM
        let prism_pair_addr = query_prismswap_prism_pair(&deps, &config, &asset.info);

        // if direct $PRISM pair does not exist, use base pair and send ust back to contract to swap it
        let (pair_addr, to_addr): (Addr, Addr) = match prism_pair_addr {
            Some(prism_pair_addr) => (prism_pair_addr, receiver.clone()),
            None => {
                // try to get the base pair from prismswap first
                // if the pair does not exist on prismswap or astroport, return error
                let base_pair_addr = query_prismswap_base_pair(&deps, &config, &asset.info)
                    .or_else(|| query_astroport_base_pair(&deps, &config, &asset.info))
                    .ok_or_else(|| {
                        StdError::generic_err(format!("Missing route for {}", asset.info))
                    })?;

                // because we are swaping to base denom,
                // we will need to call the hook to perform base->$PRISM swap
                need_hook = true;

                (base_pair_addr, env.contract.address.clone())
            }
        };

        let swap_msg = get_swap_msg(&pair_addr, &asset, &to_addr)?;
        messages.push(swap_msg);
    }

    if need_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: Some(receiver.to_string()),
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "convert_and_send")]))
}

pub fn distribute(deps: DepsMut, env: Env, asset_infos: Vec<PSAssetInfo>) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut need_hook: bool = false;
    for asset_info in &asset_infos {
        let asset_info: AssetInfo = AssetInfoUnchecked::from(asset_info).check(deps.api)?;
        let asset_balance =
            asset_info.query_balance(&deps.querier, env.contract.address.clone())?;

        if asset_balance.is_zero() {
            continue;
        }

        let asset = Asset {
            info: asset_info.clone(),
            amount: asset_balance,
        };

        let prism_pair_addr = query_prismswap_prism_pair(&deps, &config, &asset.info);
        // if direct $PRISM pair does not exist, use base pair and send ust back to contract to swap it
        let (pair_addr, to_addr): (Addr, Addr) = match prism_pair_addr {
            Some(prism_pair_addr) => (prism_pair_addr, config.distribution_contract.clone()),
            None => {
                // try to get the base pair from prismswap first
                // if the pair does not exist on prismswap or astroport, return error
                let base_pair_addr = query_prismswap_base_pair(&deps, &config, &asset.info)
                    .or_else(|| query_astroport_base_pair(&deps, &config, &asset.info))
                    .ok_or_else(|| {
                        StdError::generic_err(format!("Missing route for {}", asset.info))
                    })?;

                // because we are swapping to base denom,
                // we will need to call the hook to perform base->$PRISM swap
                need_hook = true;
                (base_pair_addr, env.contract.address.clone())
            }
        };

        let swap_msg = get_swap_msg(&pair_addr, &asset, &to_addr)?;
        messages.push(swap_msg)
    }

    if need_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook { receiver: None })?,
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
    receiver: Option<String>,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    if info.sender != env.contract.address {
        return Err(StdError::generic_err("unauthorized"));
    }

    let receiver = receiver.unwrap_or_else(|| config.distribution_contract.to_string());
    let base_asset_info = AssetInfo::Native(config.base_denom.clone());
    let balance = base_asset_info.query_balance(&deps.querier, env.contract.address)?;

    // todo - should we return an error here?
    if balance.is_zero() {
        return Ok(Response::new());
    }

    let base_asset = Asset {
        info: base_asset_info.clone(),
        amount: balance,
    };

    let prism_pair_addr = query_prismswap_prism_pair(&deps, &config, &base_asset_info)
        .ok_or_else(|| StdError::generic_err(format!("Missing route for {}", base_asset.info)))?;

    let receiver_addr = Addr::unchecked(&receiver); // already been checked
    let swap_msg = get_swap_msg(&prism_pair_addr, &base_asset, &receiver_addr)?;
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
        config.prismswap_factory.clone(),
        &[
            asset_info.into(),
            AssetInfo::Cw20(config.prism_token.clone()).into(),
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
        config.prismswap_factory.clone(),
        &[
            AssetInfo::Native(config.base_denom.clone()).into(),
            asset_info.into(),
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
                offer_asset: offer_asset.into(),
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
