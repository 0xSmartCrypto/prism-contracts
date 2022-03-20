#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, SubMsg, Uint128, WasmMsg,
};

use crate::error::{ContractError, ContractResult};
use crate::state::{CONFIG, STATE};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use prism_protocol::basset_vault::{
    BondedAmountResponse, Config, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
    QueryMsg, State, StateResponse,
};
use prism_protocol::reward_distribution::ExecuteMsg as RewardDistributionExecuteMsg;

use cw20::Cw20ReceiveMsg;

use beth::reward::ExecuteMsg as BassetRewardExecuteMsg;

const CONTRACT_NAME: &str = "prism-basset-vault";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if !info.funds.is_empty() {
        return Err(ContractError::NonPayable {});
    }
    // store config
    // TODO -- auto create token contracts from token code id
    let data = Config {
        creator: info.sender.to_string(),
        asset_contract: msg.asset_contract,
        asset_reward_contract: msg.asset_reward_contract,
        asset_reward_denom: msg.asset_reward_denom,
        casset_contract: None,
        yasset_contract: None,
        passet_contract: None,
        reward_distribution_contract: None,
    };
    CONFIG.save(deps.storage, &data)?;

    // store state
    let state = State {
        total_bond_amount: Uint128::zero(),
        last_index_modification: env.block.time.seconds(),
    };
    STATE.save(deps.storage, &state)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Split { amount } => execute_split(deps, env, info, amount),
        ExecuteMsg::Merge { amount } => execute_merge(deps, info, amount),
        ExecuteMsg::UpdateGlobalIndex {} => execute_update_global(deps, env),
        ExecuteMsg::UpdateConfig {
            owner,
            casset_contract,
            yasset_contract,
            passet_contract,
            reward_distribution_contract,
        } => execute_update_config(
            deps,
            info,
            owner,
            casset_contract,
            yasset_contract,
            passet_contract,
            reward_distribution_contract,
        ),
    }
}

/// CW20 token receive handler.
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> ContractResult<Response> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => execute_bond(deps, info, cw20_msg.amount, cw20_msg.sender),
        Ok(Cw20HookMsg::BondSplit {}) => {
            execute_bond_split(deps, env, info, cw20_msg.amount, cw20_msg.sender)
        }
        Ok(Cw20HookMsg::Unbond {}) => execute_unbond(deps, info, cw20_msg.amount, cw20_msg.sender),
        Err(_) => Err(ContractError::InvalidCw20Msg {}),
    }
}

pub fn execute_bond(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
    sender: String,
) -> ContractResult<Response> {
    let conf = CONFIG.load(deps.storage)?;
    if info.sender != conf.asset_contract {
        return Err(ContractError::Unauthorized {});
    }

    let message = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: conf.casset_contract.unwrap(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: sender,
            amount,
        })?,
        funds: vec![],
    }));

    STATE.update(deps.storage, |mut prev_state| -> StdResult<State> {
        prev_state.total_bond_amount += amount;
        Ok(prev_state)
    })?;

    Ok(Response::new().add_submessage(message).add_attributes(vec![
        attr("action", "bond"),
        attr("from", info.sender.as_str()),
        attr("minted", amount),
    ]))
}

pub fn execute_bond_split(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    sender: String,
) -> ContractResult<Response> {
    let conf = CONFIG.load(deps.storage)?;
    if info.sender != conf.asset_contract {
        return Err(ContractError::Unauthorized {});
    }

    let messages = vec![
        // mint casset for contract
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.casset_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: env.contract.address.to_string(), // mint and lock
                amount: amount,
            })?,
            funds: vec![],
        })),

        // mint yasset for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.yasset_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),

        // mint passet for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.passet_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
    ];

    STATE.update(deps.storage, |mut prev_state| -> StdResult<State> {
        prev_state.total_bond_amount += amount;
        Ok(prev_state)
    })?;

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "bond_split"),
            attr("from", info.sender.as_str()),
            attr("minted", amount),
        ]))
}

pub fn execute_unbond(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
    sender: String,
) -> ContractResult<Response> {
    let conf = CONFIG.load(deps.storage)?;
    let casset_addr = conf.casset_contract.unwrap();
    if info.sender != casset_addr {
        return Err(ContractError::Unauthorized {});
    }

    let messages = vec![
        // burn cluna from contract
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: casset_addr,
            msg: to_binary(&Cw20ExecuteMsg::Burn { amount })?,
            funds: vec![],
        })),

        // transfer basset to sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.asset_contract,
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender.clone(),
                amount,
            })?,
            funds: vec![],
        })),
    ];

    STATE.update(deps.storage, |mut prev_state| -> StdResult<State> {
        prev_state.total_bond_amount = prev_state
            .total_bond_amount
            .checked_sub(amount)
            .expect("unbond amount can not be more than stored total bonded amount");
        Ok(prev_state)
    })?;

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "burn"),
            attr("from", sender),
            attr("burnt_amount", amount),
            attr("unbonded_amount", amount),
        ]))
}

pub fn execute_split(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let messages = vec![
        // transfer casset from sender to contract
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.casset_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount,
            })?,
            funds: vec![],
        })),

        // mint yasset for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.yasset_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),

        // mint passet for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.passet_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
    ];

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "split"),
            attr("from", info.sender),
            attr("amount", amount),
        ]))
}

pub fn execute_merge(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let messages = vec![
        // transfer casset from contract to sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.casset_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),

        // burn yasset from sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.yasset_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),

        // burn passet from sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.passet_contract.unwrap(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
    ];

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "merge"),
            attr("from", info.sender),
            attr("amount", amount),
        ]))
}

pub fn execute_update_global(deps: DepsMut, env: Env) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let messages = vec![
        // claims rewards from basset reward contract using
        // reward_distribution_contract as the recipient
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.asset_reward_contract,
            msg: to_binary(&BassetRewardExecuteMsg::ClaimRewards {
                recipient: config.reward_distribution_contract.clone(),
            })
            .unwrap(),
            funds: vec![],
        })),
        // instruct reward_distribution_contract to distribute rewards
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.reward_distribution_contract.unwrap(),
            msg: to_binary(&RewardDistributionExecuteMsg::DistributeRewards {})
            .unwrap(),
            funds: vec![],
        })),
    ];

    // update state last modified
    STATE.update(deps.storage, |mut prev_state| -> StdResult<State> {
        prev_state.last_index_modification = env.block.time.seconds();
        Ok(prev_state)
    })?;

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![attr("action", "execute_update_global")]))
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    casset_contract: Option<String>,
    yasset_contract: Option<String>,
    passet_contract: Option<String>,
    reward_distribution_contract: Option<String>,
) -> ContractResult<Response> {
    // only owner must be able to send this message.
    let conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.creator {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(o) = owner {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.creator = o;
            Ok(last_config)
        })?;
    }

    if (casset_contract.is_some() && conf.casset_contract.is_some())
        || (yasset_contract.is_some() && conf.yasset_contract.is_some())
        || (passet_contract.is_some() && conf.passet_contract.is_some())
        || (reward_distribution_contract.is_some() && conf.reward_distribution_contract.is_some())
    {
        return Err(ContractError::DuplicateUpdateConfig {});
    }

    if let Some(token) = casset_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.casset_contract = Some(token);
            Ok(last_config)
        })?;
    }

    if let Some(token) = yasset_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.yasset_contract = Some(token);
            Ok(last_config)
        })?;
    }

    if let Some(token) = passet_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.passet_contract = Some(token);
            Ok(last_config)
        })?;
    }

    if let Some(reward_distribution) = reward_distribution_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.reward_distribution_contract = Some(reward_distribution.clone());
            Ok(last_config)
        })?;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::BondedAmount {} => to_binary(&query_bonded_amount(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.creator,
        asset_contract: config.asset_contract,
        asset_reward_contract: config.asset_reward_contract,
        asset_reward_denom: config.asset_reward_denom,
        casset_contract: config.casset_contract,
        yasset_contract: config.yasset_contract,
        passet_contract: config.passet_contract,
        reward_distribution_contract: config.reward_distribution_contract,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_bond_amount: state.total_bond_amount,
        last_index_modification: state.last_index_modification,
    })
}

fn query_bonded_amount(deps: Deps) -> StdResult<BondedAmountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(BondedAmountResponse {
        total_bond_amount: state.total_bond_amount,
    })
}
