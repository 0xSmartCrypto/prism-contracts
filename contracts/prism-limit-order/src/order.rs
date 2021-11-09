use cosmwasm_std::{
    attr, to_binary, Addr, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use astroport::asset::{Asset, AssetInfo};
use astroport::pair::{
    Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg, ReverseSimulationResponse,
    SimulationResponse,
};
use astroport::querier::{reverse_simulate, simulate};

use crate::state::{
    pair_key, remove_order, store_new_order, Config, OrderInfo, CONFIG, ORDERS, PAIRS,
};

pub fn submit_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    offer_asset: Asset,
    ask_asset: Asset,
) -> StdResult<Response> {
    // check if the pair exists and get the pair address
    let pair_key = pair_key(&[offer_asset.info.clone(), ask_asset.info.clone()]);
    let pair_addr: Addr = PAIRS
        .load(deps.storage, &pair_key)
        .map_err(|_| StdError::generic_err("the 2 assets provided are not supported"))?;

    let mut messages: Vec<CosmosMsg> = vec![];

    match offer_asset.info.clone() {
        AssetInfo::NativeToken { .. } => offer_asset.assert_sent_native_token_balance(&info)?,
        AssetInfo::Token { contract_addr } => {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: offer_asset.amount,
                })?,
            }));
        }
    }

    let mut new_order = OrderInfo {
        order_id: 0u64, // provisional
        bidder_addr: deps.api.addr_validate(info.sender.as_str())?,
        pair_addr: pair_addr,
        offer_asset: offer_asset.clone(),
        ask_asset: ask_asset.clone(),
    };
    store_new_order(deps.storage, &mut new_order)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "submit_order"),
        attr("order_id", new_order.order_id.to_string()),
        attr("bidder_addr", info.sender.to_string()),
        attr("offer_asset", offer_asset.to_string()),
        attr("ask_asset", ask_asset.to_string()),
    ]))
}

pub fn cancel_order(deps: DepsMut, info: MessageInfo, order_id: u64) -> StdResult<Response> {
    let order: OrderInfo = ORDERS.load(deps.storage, &order_id.to_be_bytes())?;
    if order.bidder_addr != info.sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    // refund offer asset
    let messages: Vec<CosmosMsg> = vec![order
        .offer_asset
        .clone()
        .into_msg(&deps.querier, order.bidder_addr.clone())?];

    remove_order(deps.storage, &order);

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "cancel_order"),
        attr("order_id", order_id.to_string()),
        attr("refunded_asset", order.offer_asset.to_string()),
    ]))
}

pub fn execute_order(deps: DepsMut, info: MessageInfo, order_id: u64) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    let order: OrderInfo = ORDERS.load(deps.storage, &order_id.to_be_bytes())?;

    let prism_asset_info = AssetInfo::Token {
        contract_addr: config.prism_token,
    };

    let (offer_asset, return_asset, prism_fee_amount) =
        if order.ask_asset.info.equal(&prism_asset_info) {
            // if the ask asset $PRISM, take the fee from the ask_asset
            let simul_res: SimulationResponse =
                simulate(&deps.querier, order.pair_addr.clone(), &order.offer_asset)?;
            let prism_fee_asset = Asset {
                info: prism_asset_info.clone(),
                amount: simul_res.return_amount * config.order_fee,
            };

            let sell_prism_fee_simul_res: SimulationResponse = simulate(
                &deps.querier,
                config.prism_ust_pair.clone(),
                &prism_fee_asset,
            )?;
            if sell_prism_fee_simul_res.return_amount < config.min_fee_value {
                let min_fee_asset = Asset {
                    amount: config.min_fee_value,
                    info: AssetInfo::NativeToken {
                        denom: config.base_denom,
                    },
                };
                let buy_prism_fee_simul_res: ReverseSimulationResponse =
                    reverse_simulate(&deps.querier, &config.prism_ust_pair, &min_fee_asset)?;

                (
                    order.offer_asset.clone(),
                    Asset {
                        info: prism_asset_info.clone(),
                        amount: simul_res
                            .return_amount
                            .checked_sub(buy_prism_fee_simul_res.offer_amount)?, // TODO: if the order is too small, this might fail
                    },
                    buy_prism_fee_simul_res.offer_amount,
                )
            } else {
                (
                    order.offer_asset.clone(),
                    Asset {
                        info: prism_asset_info.clone(),
                        amount: simul_res
                            .return_amount
                            .checked_sub(prism_fee_asset.amount)?,
                    },
                    prism_fee_asset.amount,
                )
            }
        } else if order.offer_asset.info.equal(&prism_asset_info) {
            // if the ask asset is not $PRISM, take the fee from the offer_asset
            let prism_fee_asset = Asset {
                info: prism_asset_info.clone(),
                amount: order.offer_asset.amount * config.order_fee,
            };
            let sell_prism_fee_simul_res: SimulationResponse = simulate(
                &deps.querier,
                config.prism_ust_pair.clone(),
                &prism_fee_asset,
            )?;

            let (offer_asset, prism_fee) =
                if sell_prism_fee_simul_res.return_amount < config.min_fee_value {
                    let min_fee_asset = Asset {
                        amount: config.min_fee_value,
                        info: AssetInfo::NativeToken {
                            denom: config.base_denom,
                        },
                    };
                    let buy_prism_fee_simul_res: ReverseSimulationResponse =
                        reverse_simulate(&deps.querier, &config.prism_ust_pair, &min_fee_asset)?;

                    (
                        Asset {
                            info: prism_asset_info.clone(),
                            amount: order
                                .offer_asset
                                .amount
                                .checked_sub(buy_prism_fee_simul_res.offer_amount)?,
                        },
                        buy_prism_fee_simul_res.offer_amount,
                    )
                } else {
                    (
                        Asset {
                            info: prism_asset_info.clone(),
                            amount: order
                                .offer_asset
                                .amount
                                .checked_sub(prism_fee_asset.amount)?,
                        },
                        prism_fee_asset.amount,
                    )
                };

            let simul_res: SimulationResponse =
                simulate(&deps.querier, order.pair_addr.clone(), &offer_asset)?;

            (
                offer_asset,
                Asset {
                    info: order.ask_asset.info.clone(),
                    amount: simul_res.return_amount,
                },
                prism_fee,
            )
        } else {
            return Err(StdError::generic_err("invalid order"));
        };

    if return_asset.amount < order.ask_asset.amount {
        return Err(StdError::generic_err("insufficient return amount"));
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    // create swap message
    match offer_asset.clone().info {
        AssetInfo::Token { contract_addr } => {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: order.pair_addr.to_string(),
                    amount: offer_asset.clone().amount,
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        to: None,
                        belief_price: None,
                        max_spread: None,
                    })?,
                })?,
            }));
        }
        AssetInfo::NativeToken { denom } => {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: order.pair_addr.to_string(),
                funds: vec![Coin {
                    denom,
                    amount: offer_asset.amount,
                }],
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset,
                    belief_price: None,
                    max_spread: None,
                    to: None,
                })?,
            }));
        }
    };

    // send asset to bidder
    messages.push(
        order
            .ask_asset
            .clone()
            .into_msg(&deps.querier, order.bidder_addr.clone())?,
    );

    // send excess to executor
    let excess_amount: Uint128 = return_asset.amount.checked_sub(order.ask_asset.amount)?;
    if excess_amount > Uint128::zero() {
        let excess_asset = Asset {
            amount: excess_amount,
            info: order.ask_asset.info.clone(),
        };
        messages.push(excess_asset.into_msg(&deps.querier, info.sender.clone())?);
    }

    // send fee to executor
    let executor_fee_asset = Asset {
        amount: prism_fee_amount * config.executor_fee_portion,
        info: prism_asset_info.clone(),
    };
    messages.push(
        executor_fee_asset
            .clone()
            .into_msg(&deps.querier, info.sender.clone())?,
    );

    // send fee to PRISM stakers
    let protocol_fee_asset = Asset {
        amount: prism_fee_amount.checked_sub(executor_fee_asset.amount)?,
        info: prism_asset_info,
    };
    // check protocol fee amount, executor_fee_portion could be 100%
    if !protocol_fee_asset.amount.is_zero() {
        messages.push(
            protocol_fee_asset
                .clone()
                .into_msg(&deps.querier, config.fee_collector_addr)?,
        );
    }

    remove_order(deps.storage, &order);

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "execute_order"),
        attr("order_id", order.order_id.to_string()),
        attr("executor_fee_amount", executor_fee_asset.amount.to_string()),
        attr("protocol_fee_amount", protocol_fee_asset.amount.to_string()),
        attr("excess_amount", excess_amount.to_string()),
    ]))
}
