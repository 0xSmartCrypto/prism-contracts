#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg
};

use prism_protocol::lp_vault::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};

use crate::error::ContractError;
use crate::state::{Config, RewardInfo, CONFIG};
use crate::query::{query_config,};

use astroport::asset::AssetInfo;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use terra_cosmwasm::TerraMsgWrapper;

pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    generator: Option<String>,
    gov: Option<String>,
    collector: Option<String>,
) -> Result<Response, ContractError> {
    // only owner must be able to send this message.
    let conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.owner {
        return Err(ContractError::Unauthorized {})
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

pub fn bond(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if !(amount > Uint128::zero()) {
        return Err(ContractError::InvalidNumTokens {});
    }

    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];

    // attempt to send LP to astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: staking_token.clone().to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: config.generator.clone(),
            msg: to_binary(&AstroHookMsg::Deposit {})?,
            amount,
        })?,
        funds: vec![],
    }));

    // update rewards for yLP stakers
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateRewards {

        })?,
        funds: vec![],
    }));

    // mint cLP tokens and update internal state
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::Mint {
            user: sender_addr.clone().to_string(),
            token: staking_token.clone().to_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    // might want some internal logic to infer [c/y/p]LP relationships
    // load BondInfo
    // err if amount < local amount OR [user, token] mapping DNE

    // we prob wanna use funds here instead of token arg
    // make sure only one token, then make sure it exists in cLP -> p/y/LP mapping
    // grab LP address

    // also need to make sure user has enough MERGED cLP 

    let withdraw_amount : Uint128 = amount.unwrap();
    let lp_addr = deps.api.addr_validate(&token)?;

    let config = CONFIG.load(deps.storage)?;
    let mut messages = vec![];

    // attempt to withdraw LP from astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator.clone(),
        msg: to_binary(&AstroExecuteMsg::Withdraw {
            lp_token: lp_addr.clone(),
            amount: withdraw_amount,
        })?,
        funds: vec![],
    }));

    // update rewards for yLP stakers
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateRewards {

        })?,
        funds: vec![],
    }));

    // burn cLP tokens and update internal state
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::Burn {
            user: info.sender.clone().to_string(),
            token: token.clone().to_string(),
            amount: withdraw_amount,
        })?,
        funds: vec![],
    }));

    // call cw20 transfer LP to user
    // need to figure out how to get LP from cLP in internal state
    let lp_addr = token;
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_addr.clone().to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.clone().to_string(),
            amount: withdraw_amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn split(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

pub fn merge(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

pub fn stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

pub fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
) -> Result<Response, ContractError> {
    // check that {user, token} RewardInfo exists
    
    // check that token is valid and safe

    // call update_rewards (calculate rewards? do we even need to store?)

    // send back all rewards

    Ok(Response::new())
}

pub fn update_staking_mode(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    mode: StakingMode,
) -> Result<Response, ContractError> {
    // check that {user, token} RewardInfo exists
    
    // check that token is valid and safe

    // update_rewards

    // update StakingMode
    Ok(Response::new())
}

pub fn mint(
    deps: DepsMut,
    env: Env, 
    info: MessageInfo,
    user: String,
    token: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // check that it is called by us
    // check that LP -> cLP exists
    // if it doesn't add instantiate message and add addr to local storage
    // push back mint message (for cLP)
    Ok(Response::new())
}

pub fn burn(
    deps: DepsMut,
    env: Env, 
    info: MessageInfo,
    user: String,
    token: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // use cw20 burnfrom
    Ok(Response::new())
}

pub fn update_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // update rewardinfo of given {user, token} after calculating fees
    // instead of updating all RewardInfo every time rewards are collected from astro generator,
    // we can probably instead look at RewardInfo's last collected and Config's collection time
    // and figure out how many cycles of rewards it collected
    // still need to figure out how this amm fee can be calculated tho/how it works
    Ok(Response::new())
}