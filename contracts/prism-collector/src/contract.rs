#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128, WasmMsg,
};

use crate::state::{Config, CONFIG};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};

use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use prismswap::asset::{Asset, AssetInfo, PairInfo};
use prismswap::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};
use prismswap::querier::{query_balance, query_pair_info, query_token_balance};

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
        ExecuteMsg::Distribute { asset_tokens } => distribute(deps, env, asset_tokens),
        ExecuteMsg::BaseSwapHook { receiver } => base_swap_hook(deps, env, info, receiver),
    }
}

pub fn convert_and_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<Asset>,
    receiver: Option<String>,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    let receiver: String = receiver.unwrap_or_else(|| info.sender.to_string());

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut need_hook: bool = false;
    for asset in assets {
        if asset.is_native_token() {
            return Err(StdError::generic_err("only accept token assets"));
        }

        // add transfer from message
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset.info.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                amount: asset.amount,
                recipient: env.contract.address.to_string(),
            })?,
            funds: vec![],
        }));

        // try to query pair with $PRISM
        let prism_pair_info_res: StdResult<PairInfo> = query_pair_info(
            &deps.querier,
            config.prismswap_factory.clone(),
            &[
                AssetInfo::Token {
                    contract_addr: config.prism_token.clone(),
                },
                asset.info.clone(),
            ],
        );

        // if direct $PRISM pair does not exist, use base pair and send ust back to contract to swap it
        let (pair_addr, to): (String, Option<String>) = match prism_pair_info_res {
            Ok(pair_info) => (pair_info.contract_addr.to_string(), Some(receiver.clone())),
            Err(_) => {
                // try to get the base pair from prismswap first
                // if the pair does not exist on prismswap or astroport, return error
                let base_pair_info: PairInfo = query_pair_info(
                    &deps.querier,
                    config.prismswap_factory.clone(),
                    &[
                        AssetInfo::NativeToken {
                            denom: config.base_denom.to_string(),
                        },
                        asset.info.clone(),
                    ],
                )
                .unwrap_or(query_pair_info(
                    &deps.querier,
                    config.astroport_factory.clone(),
                    &[
                        AssetInfo::NativeToken {
                            denom: config.base_denom.to_string(),
                        },
                        asset.info.clone(),
                    ],
                )?);

                // because we are swaping to base denom,
                // we will need to call the hook to perform base->$PRISM swap
                need_hook = true;

                (base_pair_info.contract_addr.to_string(), None)
            }
        };

        // create swap msg
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset.info.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_addr.to_string(),
                amount: asset.amount,
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to,
                })?,
            })?,
            funds: vec![],
        }));
    }

    if need_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: Some(receiver),
            })?,
            funds: vec![],
        }))
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![attr("action", "convert_and_send")]))
}

pub fn distribute(deps: DepsMut, env: Env, asset_tokens: Vec<String>) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut need_hook: bool = false;
    for asset in asset_tokens {
        let asset_addr: Addr = deps
            .api
            .addr_validate(&asset)
            .map_err(|_| StdError::generic_err("only accept token assets"))?;
        let asset_info = AssetInfo::Token {
            contract_addr: asset_addr.clone(),
        };

        let asset_balance: Uint128 =
            query_token_balance(&deps.querier, asset_addr, env.contract.address.clone())?;

        // try to query pair with $PRISM
        let prism_pair_info_res: StdResult<PairInfo> = query_pair_info(
            &deps.querier,
            config.prismswap_factory.clone(),
            &[
                AssetInfo::Token {
                    contract_addr: config.prism_token.clone(),
                },
                asset_info.clone(),
            ],
        );

        // if direct $PRISM pair does not exist, use base pair and send ust back to contract to swap it
        let (pair_addr, to): (String, Option<String>) = match prism_pair_info_res {
            Ok(pair_info) => (
                pair_info.contract_addr.to_string(),
                Some(config.distribution_contract.to_string()),
            ),
            Err(_) => {
                // try to get the base pair from prismswap first
                // if the pair does not exist on prismswap or astroport, return error
                let base_pair_info: PairInfo = query_pair_info(
                    &deps.querier,
                    config.prismswap_factory.clone(),
                    &[
                        AssetInfo::NativeToken {
                            denom: config.base_denom.to_string(),
                        },
                        asset_info.clone(),
                    ],
                )
                .unwrap_or(query_pair_info(
                    &deps.querier,
                    config.astroport_factory.clone(),
                    &[
                        AssetInfo::NativeToken {
                            denom: config.base_denom.to_string(),
                        },
                        asset_info.clone(),
                    ],
                )?);

                // because we are swaping to base denom,
                // we will need to call the hook to perform base->$PRISM swap
                need_hook = true;

                (base_pair_info.contract_addr.to_string(), None)
            }
        };

        // create swap msg
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_info.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_addr.to_string(),
                amount: asset_balance,
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to,
                })?,
            })?,
            funds: vec![],
        }));
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

    let receiver: String = receiver.unwrap_or_else(|| config.distribution_contract.to_string());

    let prism_pair_info_res: PairInfo = query_pair_info(
        &deps.querier,
        config.prismswap_factory.clone(),
        &[
            AssetInfo::NativeToken {
                denom: config.base_denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: config.prism_token.clone(),
            },
        ],
    )?;

    let amount = query_balance(
        &deps.querier,
        env.contract.address,
        config.base_denom.to_string(),
    )?;
    let swap_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: config.base_denom.clone(),
        },
        amount,
    };

    // deduct tax first
    let amount = (swap_asset.deduct_tax(&deps.querier)?).amount;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: prism_pair_info_res.contract_addr.to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount,
                    ..swap_asset
                },
                max_spread: None,
                belief_price: None,
                to: Some(receiver),
            })?,
            funds: vec![Coin {
                denom: config.base_denom,
                amount,
            }],
        }))
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
