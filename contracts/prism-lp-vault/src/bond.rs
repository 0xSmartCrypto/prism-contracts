#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

use prism_protocol::lp_vault::{ExecuteMsg, LPInfo};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};
use astroport::token::InstantiateMsg as AstroTokenInstantiateMsg;

use crate::query::{
    query_factory_config, query_generator_rewards, query_pair_info, query_token_info,
};
use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS};

use prism_common::parse_reply_instantiate_data;

use cw20::{Cw20ExecuteMsg, MinterResponse};

const CLP_INSTANTIATE_REPLY_ID: u64 = 1;
const PLP_INSTANTIATE_REPLY_ID: u64 = 2;
const YLP_INSTANTIATE_REPLY_ID: u64 = 3;
// const XYLP_INSTANTIATE_REPLY_ID = 4;

pub fn bond(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    if amount <= Uint128::zero() {
        return Err(StdError::generic_err(
            "Invalid number of LP tokens provided".to_string(),
        ));
    }

    let config = CONFIG.load(deps.storage)?;

    // attempt to send LP to astro generator
    let mut messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: staking_token.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: config.generator,
            msg: to_binary(&AstroHookMsg::Deposit {})?,
            amount,
        })?,
        funds: vec![],
    })];

    // create LP token set if it doesn't exist
    if LP_IDS.load(deps.storage, &staking_token).is_err() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::CreateTokens {
                token: staking_token.clone(),
            })?,
            funds: vec![],
        }));
    }

    // mint cLP tokens and update internal state
    messages.push(mint(
        deps,
        sender_addr.clone(),
        staking_token.clone(),
        amount,
    )?);

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "bond"),
        attr("from", sender_addr.as_str()),
        attr("LP", staking_token.as_str()),
        attr("amount", amount),
    ]))
}

pub fn unbond(
    deps: DepsMut,
    clp_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let lp_id = LP_IDS
        .load(deps.storage, &clp_token)
        .map_err(|_| StdError::generic_err("No cLP token exists".to_string()))?;
    let lp_info = LP_INFOS
        .load(deps.storage, lp_id.into())
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;
    let lp_contract = lp_info.lp_contract;

    let config = CONFIG.load(deps.storage)?;
    let messages = vec![
        // attempt to withdraw LP from astro generator
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.generator,
            msg: to_binary(&AstroExecuteMsg::Withdraw {
                lp_token: lp_contract.clone(),
                amount,
            })?,
            funds: vec![],
        }),
        // burn cLP tokens and update internal state
        burn(deps, sender_addr.clone(), clp_token, amount)?,
        // call cw20 transfer LP to user
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender_addr.to_string(),
                amount,
            })?,
            funds: vec![],
        }),
    ];

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "unbond"),
        attr("from", sender_addr.as_str()),
        attr("LP", lp_contract.as_str()),
        attr("amount", amount),
    ]))
}

pub fn mint(deps: DepsMut, user: Addr, token: Addr, amount: Uint128) -> StdResult<CosmosMsg> {
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No cLP token exists".to_string()))?;
    let mut lp_info = LP_INFOS
        .load(deps.storage, lp_id.into())
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;

    // update internal state
    lp_info.amt_bonded += amount;
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info)?;

    // mint cLP to user
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: user.to_string(),
            amount,
        })?,
        funds: vec![],
    }))
}

pub fn burn(deps: DepsMut, user: Addr, token: Addr, amount: Uint128) -> StdResult<CosmosMsg> {
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No cLP token exists".to_string()))?;
    let mut lp_info = LP_INFOS
        .load(deps.storage, lp_id.into())
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;

    // update internal state
    lp_info.amt_bonded = lp_info.amt_bonded.checked_sub(amount)?;
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info)?;

    // burn cLP from user
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: user.to_string(),
            amount,
        })?,
        funds: vec![],
    }))
}

pub fn create_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
) -> StdResult<Response> {
    if info.sender.as_str() != env.contract.address {
        return Err(StdError::generic_err("Unauthorized".to_string()));
    }

    // Get relevant info to create new LP token set
    let new_lp_id = NUM_LPS.load(deps.storage)?;
    let pair_info = query_pair_info(deps.as_ref(), &deps.querier, token.clone())?;
    let factory_config = query_factory_config(deps.as_ref(), &deps.querier)?;
    let token_info = query_token_info(&deps.querier, token.clone())?;
    let generator_rewards = query_generator_rewards(deps.as_ref(), &deps.querier, token.clone())?;

    // Store new base LP token mapping
    LP_IDS.save(deps.storage, &token, &new_lp_id.clone())?;

    // Store id -> LPInfo mapping
    let new_lp_info = LPInfo {
        pair_asset_info: pair_info.asset_infos.clone(),
        generator_reward_info: generator_rewards,
        amt_bonded: Uint128::zero(),
        last_liquidity: Decimal::zero(),
        pair_contract: pair_info.contract_addr,
        lp_contract: token,
        clp_contract: Addr::unchecked("".to_string()),
        plp_contract: Addr::unchecked("".to_string()),
        ylp_contract: Addr::unchecked("".to_string()),
    };
    LP_INFOS.save(deps.storage, new_lp_id.into(), &new_lp_info)?;

    // Instantiate new tokens
    // we will generalize this for other AMM's in the future
    let sub_msg = vec![
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: factory_config.token_code_id,
                msg: to_binary(&AstroTokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "c".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "c".to_string())?,
                    decimals: token_info.decimals,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                })?,
                funds: vec![],
                admin: None,
                label: String::from("Prism cLP token"),
            }
            .into(),
            id: CLP_INSTANTIATE_REPLY_ID,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: factory_config.token_code_id,
                msg: to_binary(&AstroTokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "p".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "p".to_string())?,
                    decimals: token_info.decimals,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                })?,
                funds: vec![],
                admin: None,
                label: String::from("Prism pLP token"),
            }
            .into(),
            id: PLP_INSTANTIATE_REPLY_ID,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: factory_config.token_code_id,
                msg: to_binary(&AstroTokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "y".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "y".to_string())?,
                    decimals: token_info.decimals,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                })?,
                funds: vec![],
                admin: None,
                label: String::from("Prism yLP token"),
            }
            .into(),
            id: YLP_INSTANTIATE_REPLY_ID,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
    ];
    Ok(Response::new().add_submessages(sub_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
#[allow(dead_code)] // throws warnings on compile because we don't call reply explicitly
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    // grab address from data field and validate
    let id = msg.id;
    let res = parse_reply_instantiate_data(msg)
        .map_err(|_| StdError::generic_err("Failed to parse reply"))?;
    
    // get LPInfo to modify
    let new_token_addr = deps.api.addr_validate(&res.contract_address)?;
    let lp_id = NUM_LPS.load(deps.storage)?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    // save new addr -> id mapping
    LP_IDS.save(deps.storage, &new_token_addr, &lp_id.clone())?;

    // update the correct contract
    match id {
        CLP_INSTANTIATE_REPLY_ID => {
            lp_info.clp_contract = new_token_addr;
        }
        PLP_INSTANTIATE_REPLY_ID => {
            lp_info.plp_contract = new_token_addr;
        }
        YLP_INSTANTIATE_REPLY_ID => {
            lp_info.ylp_contract = new_token_addr;

            // update LP id on last token instantiation
            NUM_LPS.update(deps.storage, |lp_id| -> StdResult<u64> { Ok(lp_id + 1) })?;
        }
        _ => {
            return Err(StdError::generic_err("Bad Reply ID".to_string()));
        }
    };

    // save new contract
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info)?;
    Ok(Response::new())
}

// ??
// pain
// QUES: is there any better way to do this string manip
pub fn format_token_name(name: &str, option: String) -> StdResult<String> {
    // "{}-{}-LP" --> "{}-{}-[c/p/y]LP"
    let index = name.rfind('-');

    if index == None {
        return Err(StdError::generic_err("format token name issue"));
    }

    let mut test = name.to_string();
    test.insert_str(index.unwrap(), &option);
    Ok(test)
}

pub fn format_token_symbol(symbol: &str, option: String) -> StdResult<String> {
    // "uLP" --> "[c/p/y]uLP"
    Ok(option + symbol)
}
