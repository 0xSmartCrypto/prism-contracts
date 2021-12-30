#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg, Reply, ReplyOn, Decimal,
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, RewardInfo, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};
use astroport::token::{InstantiateMsg as AstroTokenInstantiateMsg};
use astroport::factory::{ConfigResponse as FactoryConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS, LPInfo};
use crate::query::{query_config, query_token_info, query_pair_info, query_factory_config};

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

// only executable by owner
pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    generator: Option<String>,
    factory: Option<String>,
    collector: Option<String>,
) -> StdResult<Response> {
    let conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.owner {
        return Err(StdError::generic_err(format!("Unauthorized")));
    }

    if let Some(creator) = owner {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.owner = creator;
            Ok(last_config)
        })?;
    }

    if let Some(generator_contract) = generator {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.generator = generator_contract;
            Ok(last_config)
        })?;
    }

    if let Some(factory_contract) = factory {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.factory = factory_contract;
            Ok(last_config)
        })?;
    }

    if let Some(fee_contract) = collector {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.collector = fee_contract;
            Ok(last_config)
        })?;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

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
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::CreateTokens { token: staking_token.clone() })?,
        funds: vec![],
    }));

    // update rewards for yLP stakers
    // can we move when this is done to save computation? maybe when users query rewards? (lazily)
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateRewards { })?,
        funds: vec![],
    }));

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

    // update rewards for yLP stakers
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateRewards { })?,
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

pub fn split(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Uint128,
) -> StdResult<Response> {
    // make sure cLP token exists
    let token_addr = Addr::unchecked(token);
    let lp_id = LP_IDS.load(deps.storage, &token_addr)
                      .map_err(|_| StdError::generic_err(format!("No cLP address exists")))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())
                              .map_err(|_| StdError::generic_err(format!("No cLP address exists")))?;

    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.plp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn merge(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Uint128,
) -> StdResult<Response> {
    let token_addr = Addr::unchecked(token);
    let lp_id = LP_IDS.load(deps.storage, &token_addr)
                      .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())
                              .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;

    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.plp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

// TODO
// should be cw20
pub fn stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    // yLP has already been transfered because cw20 send
    // params will be ylp_contract, sender_addr, amount

    // call update rewards
    // check for (lp_id, user) staker_info
    // if exists, add bond amount
    // else, create new StakerInfo with bond amount and store
    Ok(Response::new())
}

// TODO
pub fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    // params will be ylp_contract, info.sender, Option<amount>
    
    // call update rewards
    // check for (lp_id, user) staker_info
    // if doesn't exist or amount < whats available, error
    // if amount is empty, do all bonded yLP
    // if bond amount is empty and RewardInfo is empty, delete StakerInfo instance
    Ok(Response::new())
}

// TODO
pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    // call update_rewards

    // for each {info.sender, token_id} in STAKER_INFO

    // send back all rewards (make a helper per RewardInfo)

    // delete StakerInfo instance iff amt_bonded is empty

    Ok(Response::new())
}

// TODO
pub fn update_staking_mode(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    mode: StakingMode,
) -> StdResult<Response> {
    // call update_rewards

    // send tokens

    // check that {user, token} StakerInfo exists

    // update StakingMode
    Ok(Response::new())
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

// TODO
pub fn create_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
) -> StdResult<Response> {
    let new_lp_id = NUM_LPS.load(deps.storage)? + 1;
    
    // Get pair contract info and asset info
    let pair_info = query_pair_info(deps.as_ref(), &deps.querier, token.clone())?;

    // Get factory config for token code id
    let factory_config = query_factory_config(deps.as_ref(), &deps.querier)?;

    // Get LP token name for naming the new tokens
    let token_info = query_token_info(&deps.querier, token.clone())?;

    // Store LP address -> id mapping
    LP_IDS.save(deps.storage, &token.clone(), &new_lp_id.clone())?;

    // Store id -> LPInfo mapping with lp_contract
    let new_lp_info = LPInfo {
        asset_infos: pair_info.asset_infos,
        amt_bonded: Uint128::zero(),
        last_liquidity: Decimal::zero(),
        pair_contract: pair_info.contract_addr,
        lp_contract: token,
        clp_contract: Addr::unchecked("".to_string()),
        plp_contract: Addr::unchecked("".to_string()),
        ylp_contract: Addr::unchecked("".to_string()),
    };

    // LP token name is of form "{}-{}-LP"
    // symbol is "uLP"

    // new token names will be:
    // "{}-{}-[c/p/y]LP"
    // "cLP"
    // make the helper string formatting functions in lp_vault.rs
    let clp_name = token_info.name.clone();
    let plp_name = token_info.name.clone();
    let ylp_name = token_info.name.clone();

    let clp_symbol = token_info.symbol.clone();
    let plp_symbol = token_info.symbol.clone();
    let ylp_symbol = token_info.symbol.clone();

    // Instantiate new tokens (tentative cw20 format)
    // we should probably make our own cw20 LP's intead for c/y/pLP's, this is just easiest for now
    let sub_msg: Vec<SubMsg> = vec![
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: factory_config.token_code_id,
                msg: to_binary(&AstroTokenInstantiateMsg {
                    name: clp_name,
                    symbol: clp_symbol,
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
                    name: plp_name,
                    symbol: plp_symbol,
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
                    name: ylp_name,
                    symbol: ylp_symbol,
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
    

    // will this reset if any of the instantiatemsg's here fail? or should the jank
    // temp storage item be used
    // prob the temp storage will have to be used

    // figure this out later
    NUM_LPS.save(deps.storage, &new_lp_id)?;
    Ok(Response::new().add_submessages(sub_msg))
}

// TODO
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    // grab address from data field and validate
    let mut config: Config = CONFIG.load(deps.storage)?;

    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    let new_token_addr = addr_validate_to_lower(deps.api, res.get_contract_address())?;
    let lp_id = NUM_LPS.load(deps.storage)?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;
    
    match msg.id {
        CLP_INSTANTIATE_REPLY_ID => { lp_info.clp_contract = new_token_addr; },
        PLP_INSTANTIATE_REPLY_ID => { lp_info.plp_contract = new_token_addr; },
        YLP_INSTANTIATE_REPLY_ID => { lp_info.ylp_contract = new_token_addr; },
        _ => { return Err(StdError::generic_err(format!("Bad Reply ID"))); }
    };
    // filter by reply ID
    // create [c/p/y]LP -> id mapping, update LPInfo in id -> LPInfo
    Ok(Response::new())
}

// TODO
pub fn update_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    // update RewardInfo of given LP token

    // grab x and y from Astroport, amt_bonded and last liquidity from LP_INFO
    // calculate new liquidity, calculate amount of LP to withdraw and burn

    // for each {user, token_id} in STAKER_INFO
    // if default mode, add underlying rewards
    // if xprism mode, use collector contract to convert and add to xprism reward

    Ok(Response::new())
}