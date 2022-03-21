#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Attribute, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Reply, ReplyOn, Response, StdResult, SubMsg, Uint128, WasmMsg,
};

use crate::error::{ContractError, ContractResult};
use crate::state::{Config, State, CONFIG, STATE};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use prism_protocol::basset_vault::{
    BondedAmountResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg,
    StateResponse,
};
use prism_protocol::internal::parse_reply_instantiate_data;
use prism_protocol::reward_distribution::ExecuteMsg as RewardDistributionExecuteMsg;
use prismswap::token::InstantiateMsg as TokenInstantiateMsg;

use cw20::{Cw20ReceiveMsg, MinterResponse};

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
    let config = Config {
        owner: info.sender,
        asset_name: msg.asset_name,
        asset_contract: deps.api.addr_validate(&msg.asset_contract)?,
        asset_reward_contract: deps.api.addr_validate(&msg.asset_reward_contract)?,
        asset_reward_denom: msg.asset_reward_denom,
        casset_contract: Addr::unchecked(""),
        yasset_contract: Addr::unchecked(""),
        passet_contract: Addr::unchecked(""),
        reward_distribution_contract: Addr::unchecked(""),
        initialized: false,
        token_admin: deps.api.addr_validate(&msg.token_admin)?,
        token_code_id: msg.token_code_id,
    };
    CONFIG.save(deps.storage, &config)?;

    // store state
    let state = State {
        total_bond_amount: Uint128::zero(),
        last_index_modification: env.block.time.seconds(),
    };
    STATE.save(deps.storage, &state)?;

    // start initialization of 3 tokens, cAsset -> passet -> yAsset
    let message = SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: config.token_code_id,
            msg: to_binary(&TokenInstantiateMsg {
                name: format!("Prism c{} Token", config.asset_name),
                symbol: format!("c{}", config.asset_name),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None,
                }),
            })?,
            funds: vec![],
            admin: Some(config.token_admin.to_string()),
            label: "".to_string(),
        }
        .into(),
        id: 0u64,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    Ok(Response::new().add_submessage(message))
}

/// Replies received after token instantiation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> ContractResult<Response> {
    set_token_address(deps, env, msg)
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
            reward_distribution_contract,
        } => execute_update_config(deps, info, owner, reward_distribution_contract),
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
    if info.sender != conf.asset_contract {
        return Err(ContractError::Unauthorized {});
    }

    let message = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: conf.casset_contract.to_string(),
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
    if info.sender != conf.asset_contract {
        return Err(ContractError::Unauthorized {});
    }

    let messages = vec![
        // mint casset for contract
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.casset_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: env.contract.address.to_string(), // mint and lock
                amount,
            })?,
            funds: vec![],
        })),
        // mint yasset for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.yasset_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
        // mint passet for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.passet_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender,
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
    let casset_addr = conf.casset_contract.to_string();
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
            contract_addr: conf.asset_contract.to_string(),
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;

    let messages = vec![
        // transfer casset from sender to contract
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.casset_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
        // mint yasset for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.yasset_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
        // mint passet for sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.passet_contract.to_string(),
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
    let messages = vec![
        // transfer casset from contract to sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.casset_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
        // burn yasset from sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.yasset_contract.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        })),
        // burn passet from sender
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.passet_contract.to_string(),
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
    let messages = vec![
        // claims rewards from basset reward contract using
        // reward_distribution_contract as the recipient
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.asset_reward_contract.to_string(),
            msg: to_binary(&BassetRewardExecuteMsg::ClaimRewards {
                recipient: Some(conf.reward_distribution_contract.to_string()),
            })
            .unwrap(),
            funds: vec![],
        })),
        // instruct reward_distribution_contract to distribute rewards
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.reward_distribution_contract.to_string(),
            msg: to_binary(&RewardDistributionExecuteMsg::DistributeRewards {}).unwrap(),
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
    reward_distribution_contract: Option<String>,
) -> ContractResult<Response> {
    // only owner must be able to send this message.
    let mut conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(o) = owner {
        conf.owner = deps.api.addr_validate(&o)?;
    }

    if let Some(token) = reward_distribution_contract {
        conf.reward_distribution_contract = deps.api.addr_validate(&token)?;
    }

    let placeholder_addr = Addr::unchecked("");
    if !conf.initialized && conf.reward_distribution_contract.ne(&placeholder_addr) {
        conf.initialized = true;
    }

    CONFIG.save(deps.storage, &conf)?;

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

    Ok(config.as_res())
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;

    Ok(state.as_res())
}

fn query_bonded_amount(deps: Deps) -> StdResult<BondedAmountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(BondedAmountResponse {
        total_bond_amount: state.total_bond_amount,
    })
}

pub fn set_token_address(deps: DepsMut, env: Env, msg: Reply) -> ContractResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    let res =
        parse_reply_instantiate_data(msg.clone()).map_err(|_| ContractError::ParseReplyError {})?;
    let token_addr = deps.api.addr_validate(&res.contract_address)?;

    let mut attributes: Vec<Attribute> = vec![];
    let (next_reply_id, next_token_name, next_token_symbol) = match msg.id {
        0 => {
            attributes.push(attr("casset_address", token_addr.as_str()));
            config.casset_contract = token_addr;
            let next_token_symbol = format!("p{}", config.asset_name);

            (
                1u64,
                format!("Prism {} Token", next_token_symbol),
                next_token_symbol,
            )
        }
        1 => {
            attributes.push(attr("passet_address", token_addr.as_str()));
            config.passet_contract = token_addr;

            let next_token_symbol = format!("y{}", config.asset_name);
            (
                2u64,
                format!("Prism {} Token", next_token_symbol),
                next_token_symbol,
            )
        }
        2 => {
            attributes.push(attr("yasset_address", token_addr.as_str()));
            config.yasset_contract = token_addr;

            (3u64, "".to_string(), "".to_string())
        }
        _ => return Err(ContractError::InvalidReplayId {}),
    };

    CONFIG.save(deps.storage, &config)?;

    let mut messages: Vec<SubMsg> = vec![];
    if next_reply_id <= 2 {
        messages.push(SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: config.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: next_token_name,
                    symbol: next_token_symbol,
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                })?,
                funds: vec![],
                admin: Some(config.token_admin.to_string()),
                label: "".to_string(),
            }
            .into(),
            id: next_reply_id,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        })
    }

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(attributes))
}
