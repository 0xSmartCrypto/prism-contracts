#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg, Reply, ReplyOn, Decimal,
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};
use astroport::token::{InstantiateMsg as AstroTokenInstantiateMsg};
use astroport::factory::{ConfigResponse as FactoryConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS, LPInfo};
use crate::query::{query_config, query_token_info, query_pair_info, query_factory_config, query_generator_rewards};

use crate::response::MsgInstantiateContractResponse;
use protobuf::Message;

use astroport::asset::{AssetInfo, addr_validate_to_lower};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, TokenInfoResponse, MinterResponse};
use terra_cosmwasm::TerraMsgWrapper;

// used for reply calls
const CLP_INSTANTIATE_REPLY_ID: u64 = 1;
const PLP_INSTANTIATE_REPLY_ID: u64 = 2;
const YLP_INSTANTIATE_REPLY_ID: u64 = 3;
// const XYLP_INSTANTIATE_REPLY_ID = 4;

// only callable by cw20
pub fn bond(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    if !(amount > Uint128::zero()) {
        return Err(StdError::generic_err(format!("Invalid number of LP tokens provided")));
    }

    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];

    // attempt to send LP to astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: staking_token.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: config.generator.clone(),
            msg: to_binary(&AstroHookMsg::Deposit {})?,
            amount,
        })?,
        funds: vec![],
    }));

    // create LP token set if it doesn't exist
    if LP_IDS.may_load(deps.storage, &staking_token)? == None {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::CreateTokens { token: staking_token.clone() })?,
            funds: vec![],
        }));
    }

    // mint cLP tokens and update internal state
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::Mint {
            user: sender_addr.clone().to_string(),
            token: staking_token.clone(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

// only callable by cw20
pub fn unbond(
    deps: DepsMut,
    env: Env,
    clp_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    // make sure cLP token exists
    let lp_id = LP_IDS.load(deps.storage, &clp_token.clone())
                            .map_err(|_| StdError::generic_err(format!("No cLP address exists")))?;
    // grab LP address
    // this shouldn't fail
    let lp_info = LP_INFOS.load(deps.storage, lp_id.clone().into())
                              .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let lp_contract = lp_info.lp_contract;

    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];

    // attempt to withdraw LP from astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator.clone(),
        msg: to_binary(&AstroExecuteMsg::Withdraw {
            lp_token: lp_contract.clone(),
            amount,
        })?,
        funds: vec![],
    }));

    // burn cLP tokens and update internal state
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::Burn {
            user: sender_addr.clone().into_string(),
            token: clp_token.clone(),
            amount,
        })?,
        funds: vec![],
    }));

    // call cw20 transfer LP to user
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_contract.clone().to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: sender_addr.clone().to_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn mint(
    deps: DepsMut,
    env: Env, 
    info: MessageInfo,
    user: String,
    token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    // check that it is called by us
    if info.sender.as_str() != env.contract.address.to_string() {
        return Err(StdError::generic_err(format!("Unauthorized")));
    }

    // these should never fail
    let lp_id = LP_IDS.load(deps.storage, &token.clone())
                            .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.clone().into())
                            .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    
    // update internal state
    lp_info.amt_bonded += amount;
    LP_INFOS.save(deps.storage, lp_id.clone().into(), &lp_info)?;

    // mint cLP to user
    Ok(Response::new()
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lp_info.clp_contract.clone().into_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: user.clone(),
                    amount,
                })?,
                funds: vec![],
            }),
        ])
    )
}

pub fn burn(
    deps: DepsMut,
    env: Env, 
    info: MessageInfo,
    user: String,
    token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    // check that it is called by us
    if info.sender.as_str() != env.contract.address.to_string() {
        return Err(StdError::generic_err(format!("Unauthorized")));
    }

    // these should never fail
    let lp_id = LP_IDS.load(deps.storage, &token.clone())
                        .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.clone().into())
                              .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    
    lp_info.amt_bonded -= amount;
    LP_INFOS.save(deps.storage, lp_id.clone().into(), &lp_info)?;

    // burn cLP from user
    Ok(Response::new()
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lp_info.clp_contract.clone().into_string(),
                msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                    owner: user.clone(),
                    amount,
                })?,
                funds: vec![],
            }),
        ])
    )
}

pub fn create_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
) -> StdResult<Response> {
    // check that it is called by us
    if info.sender.as_str() != env.contract.address.to_string() {
        return Err(StdError::generic_err(format!("Unauthorized")));
    }

    let new_lp_id = NUM_LPS.load(deps.storage)?;
    
    // Get relevant info
    let pair_info = query_pair_info(deps.as_ref(), &deps.querier, token.clone())?;
    let factory_config = query_factory_config(deps.as_ref(), &deps.querier)?;
    let token_info = query_token_info(&deps.querier, token.clone())?;
    let generator_rewards = query_generator_rewards(deps.as_ref(), &deps.querier, token.clone())?;

    // Store new token mappings
    LP_IDS.save(deps.storage, &token.clone(), &new_lp_id.clone())?;

    // Store id -> LPInfo mapping with lp_contract
    let new_lp_info = LPInfo {
        pair_asset_info: pair_info.asset_infos.clone(),
        generator_reward_info: generator_rewards.clone(),
        amt_bonded: Uint128::zero(),
        last_liquidity: Decimal::zero(),
        pair_contract: pair_info.contract_addr,
        lp_contract: token,
        clp_contract: Addr::unchecked("".to_string()),
        plp_contract: Addr::unchecked("".to_string()),
        ylp_contract: Addr::unchecked("".to_string()),
    };
    LP_INFOS.save(deps.storage, new_lp_id.clone().into(), &new_lp_info);

    // Instantiate new tokens
    // we will make our own cw20 LP's intead for c/y/pLP's to generalize per AMM, 
    // this is just easiest for astroport for now
    let sub_msg: Vec<SubMsg> = vec![
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: factory_config.token_code_id,
                msg: to_binary(&AstroTokenInstantiateMsg {
                    name: format_token_name(&token_info.name.clone(), "c".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol.clone(), "c".to_string())?,
                    decimals: token_info.decimals.clone(),
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
                    name: format_token_name(&token_info.name.clone(), "p".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol.clone(), "p".to_string())?,
                    decimals: token_info.decimals.clone(),
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
                    name: format_token_name(&token_info.name.clone(), "y".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol.clone(), "y".to_string())?,
                    decimals: token_info.decimals.clone(),
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
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    // grab address from data field and validate
    let config: Config = CONFIG.load(deps.storage)?;

    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    // get LPInfo to modify
    let new_token_addr = addr_validate_to_lower(deps.api, res.get_contract_address())?;
    let lp_id = NUM_LPS.load(deps.storage)?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;
    
    // save new addr -> id mapping
    LP_IDS.save(deps.storage, &new_token_addr, &lp_id.clone())?;

    // update the correct contract
    // we can turn change lp storage in LPInfo to a vec to clean up this logic a bit
    // and make it more extensible to add xyLP (and other derivatives) in the future
    match msg.id {
        CLP_INSTANTIATE_REPLY_ID => { 
            // check if cLP has already been instantiated
            if lp_info.clp_contract != Addr::unchecked("") {
                return Err(StdError::generic_err("Unauthorized"));
            }

            lp_info.clp_contract = new_token_addr; 
        },
        PLP_INSTANTIATE_REPLY_ID => { 
            // check if pLP has already been instantiated
            if lp_info.plp_contract != Addr::unchecked("") {
                return Err(StdError::generic_err("Unauthorized"));
            }

            lp_info.plp_contract = new_token_addr; 
        },
        YLP_INSTANTIATE_REPLY_ID => {
            // check if yLP has already been instantiated
            if lp_info.ylp_contract != Addr::unchecked("") {
                return Err(StdError::generic_err("Unauthorized"));
            }

            // update LP id on last token instantiation
            lp_info.ylp_contract = new_token_addr; 
            NUM_LPS.save(deps.storage, &(lp_id + 1))?;
        },
        _ => { return Err(StdError::generic_err(format!("Bad Reply ID"))); }
    };

    // save new contract
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info)?;
    Ok(Response::new())
}

// ??
// pain
pub fn format_token_name(name: &String, option: String) -> StdResult<String> {
    // "{}-{}-LP" --> "{}-{}-[c/p/y]LP"
    let index = name.rfind('-');

    if index == None {
        return Err(StdError::generic_err("format token name issue"));
    }

    let mut test = name.clone();
    test.insert_str(index.unwrap(), &option);
    Ok(test)
}

pub fn format_token_symbol(symbol: &String, option: String) -> StdResult<String> {
    // "uLP" --> "[c/p/y]uLP"
    Ok(option + symbol)
}