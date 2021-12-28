#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, RewardInfo, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS};
use crate::query::{query_config,};

use astroport::asset::AssetInfo;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use terra_cosmwasm::TerraMsgWrapper;

// only executable by owner
pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    generator: Option<String>,
    gov: Option<String>,
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

    if let Some(gov_contract) = gov {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.gov = gov_contract;
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
        msg: to_binary(&ExecuteMsg::CreateTokens { })?,
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
    let lp_addr = lp_info.lp_addr;

    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];

    // attempt to withdraw LP from astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator.clone(),
        msg: to_binary(&AstroExecuteMsg::Withdraw {
            lp_token: lp_addr.clone(),
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
        contract_addr: lp_addr.clone().to_string(),
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
        contract_addr: lp_info.clp_addr.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.plp_addr.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_addr.clone().into_string(),
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
        contract_addr: lp_info.plp_addr.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_addr.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: info.sender.clone().into_string(),
            amount,
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_addr.clone().into_string(),
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
    // params will be ylp_addr, sender_addr, amount

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
    // params will be ylp_addr, info.sender, Option<amount>
    
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
                contract_addr: lp_info.clp_addr.clone().into_string(),
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
                contract_addr: lp_info.clp_addr.clone().into_string(),
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
) -> StdResult<Response> {
    let new_lp_id = NUM_LPS.load(deps.storage)?;
    // TODO:
    // let mut messages = vec![];
    // add TokenInstantiateMsg for cLP
    // add TokenInstantiateMsg for pLP
    // add TokenInstantiateMsg for yLP

    // maybe add xyLP in the future
    NUM_LPS.save(deps.storage, &(new_lp_id + 1))?;
    Ok(Response::new())
}

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

pub fn post_initialize(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    // TODO:
    // add new token info to internal DS
    // might need to do more here
    Ok(Response::new())
}