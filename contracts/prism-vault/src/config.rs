use crate::{
    contract::validate_rate,
    state::{
        is_valid_validator, read_validators, remove_white_validators, store_white_validators,
        Parameters, CONFIG, PARAMETERS,
    },
};
use cosmwasm_std::{
    attr, to_binary, Addr, Attribute, Coin, CosmosMsg, Decimal, DepsMut, DistributionMsg, Env,
    MessageInfo, Reply, ReplyOn, Response, StakingMsg, StdError, StdResult, SubMsg, Uint128,
    WasmMsg,
};
use cw20::MinterResponse;
use prism_protocol::{internal::parse_reply_instantiate_data, vault::ExecuteMsg};
use prismswap::token::InstantiateMsg as TokenInstantiateMsg;

pub const MAX_VALIDATORS: u64 = 20;

pub fn set_token_address(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    let res = parse_reply_instantiate_data(msg.clone())
        .map_err(|_| StdError::generic_err("error parsing token instantiation reply"))?;
    let token_addr = deps.api.addr_validate(&res.contract_address)?;

    let mut attributes: Vec<Attribute> = vec![];
    let (next_reply_id, next_token_name, next_token_symbol) = match msg.id {
        0 => {
            attributes.push(attr("cluna_address", token_addr.as_str()));
            config.cluna_contract = token_addr;

            (1u64, "Prism pLuna Token", "pLuna")
        }
        1 => {
            attributes.push(attr("pluna_address", token_addr.as_str()));
            config.pluna_contract = token_addr;

            (2u64, "Prism yLuna Token", "yLuna")
        }
        2 => {
            attributes.push(attr("yluna_address", token_addr.as_str()));
            config.yluna_contract = token_addr;

            (3u64, "", "")
        }
        _ => return Err(StdError::generic_err("invalid reply id")),
    };

    CONFIG.save(deps.storage, &config)?;

    let mut messages: Vec<SubMsg> = vec![];
    if next_reply_id <= 2 {
        messages.push(SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: config.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: next_token_name.to_string(),
                    symbol: next_token_symbol.to_string(),
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

/// Update general parameters
/// Only creator/owner is allowed to execute
#[allow(clippy::too_many_arguments)]
pub fn execute_update_params(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    epoch_period: Option<u64>,
    unbonding_period: Option<u64>,
    peg_recovery_fee: Option<Decimal>,
    er_threshold: Option<Decimal>,
) -> StdResult<Response> {
    // only owner can send this message.
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let params: Parameters = PARAMETERS.load(deps.storage)?;

    let new_params = Parameters {
        epoch_period: epoch_period.unwrap_or(params.epoch_period),
        underlying_coin_denom: params.underlying_coin_denom,
        unbonding_period: unbonding_period.unwrap_or(params.unbonding_period),
        peg_recovery_fee: validate_rate(peg_recovery_fee.unwrap_or(params.peg_recovery_fee))?,
        er_threshold: validate_rate(er_threshold.unwrap_or(params.er_threshold))?,
    };

    PARAMETERS.save(deps.storage, &new_params)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_params")]))
}

/// Update the config. Update the owner, reward and airdrop contract
/// Also used to post initialize the contract
/// Only creator/owner is allowed to execute
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    reward_distribution: Option<String>,
    delegator_rewards_contract: Option<String>,
    airdrop_registry_contract: Option<String>,
    manager: Option<String>,
) -> StdResult<Response> {
    // only owner must be able to send this message.
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut messages: Vec<SubMsg> = vec![];

    if let Some(owner) = owner {
        config.owner = deps.api.addr_validate(&owner)?;
    }

    if let Some(reward_distribution) = reward_distribution {
        config.reward_distribution = deps.api.addr_validate(&reward_distribution)?;
    }

    if let Some(delegator_rewards_contract) = delegator_rewards_contract {
        config.delegator_rewards_contract = deps.api.addr_validate(&delegator_rewards_contract)?;

        // register the reward contract for automated reward withdrawal.
        messages.push(SubMsg::new(CosmosMsg::Distribution(
            DistributionMsg::SetWithdrawAddress {
                address: delegator_rewards_contract,
            },
        )));
    }

    if let Some(airdrop) = airdrop_registry_contract {
        config.airdrop_registry_contract = deps.api.addr_validate(&airdrop)?;
    }

    if let Some(manager) = manager {
        config.manager = deps.api.addr_validate(&manager)?;
    }

    let placeholder_addr = Addr::unchecked("");
    if !config.initialized
        && config.reward_distribution.ne(&placeholder_addr)
        && config.airdrop_registry_contract.ne(&placeholder_addr)
    {
        config.initialized = true;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![attr("action", "update_config")]))
}

/// Register a white listed validator.
/// Only creator/owner is allowed to execute
pub fn execute_register_validator(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner && info.sender != env.contract.address {
        return Err(StdError::generic_err("unauthorized"));
    }

    // check if validator count exceeds max
    if read_validators(deps.storage)?.len() >= MAX_VALIDATORS as usize {
        return Err(StdError::generic_err(format!(
            "Can't register more than {} validators",
            MAX_VALIDATORS
        )));
    }

    // given validator must be first a validator in the system.
    let exists = deps
        .querier
        .query_all_validators()?
        .iter()
        .any(|val| val.address == validator);
    if !exists {
        return Err(StdError::generic_err(
            "The specified address is not a validator",
        ));
    }

    let validator_addr = Addr::unchecked(&validator);
    store_white_validators(deps.storage, &validator_addr)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register_validator"),
        attr("validator", validator),
    ]))
}

/// Deregister a previously-whitelisted validator.
/// Only creator/owner is allowed to execute
pub fn execute_deregister_validator(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator: String,
    redel_validator: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?.assert_initialized()?;
    let validator_addr = Addr::unchecked(&validator);
    let redel_validator = Addr::unchecked(&redel_validator);

    if config.owner != info.sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    let validators_before_remove = read_validators(deps.storage)?;
    if validators_before_remove.len() == 1 {
        return Err(StdError::generic_err(
            "Cannot remove the last whitelisted validator",
        ));
    }

    remove_white_validators(deps.storage, &validator_addr)?;

    let query = deps
        .querier
        .query_delegation(env.contract.address.clone(), validator.clone());

    let is_redel_valid = is_valid_validator(deps.storage, &redel_validator)?;
    if !is_redel_valid {
        return Err(StdError::generic_err(
            "The chosen validator to redelegate is currently not supported",
        ));
    }

    let mut messages: Vec<SubMsg> = vec![];

    if let Ok(q) = query {
        let delegated_amount = q;

        if let Some(delegation) = delegated_amount {
            messages.push(SubMsg::new(CosmosMsg::Staking(StakingMsg::Redelegate {
                src_validator: validator.to_string(),
                dst_validator: redel_validator.to_string(),
                amount: delegation.amount,
            })));

            let msg = ExecuteMsg::UpdateGlobalIndex {
                airdrop_hooks: None,
            };
            messages.push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&msg)?,
                funds: vec![],
            })));
        }
    }

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "de_register_validator"),
            attr("validator", validator),
            attr("redel_validator", redel_validator),
        ]))
}

/// This operation can only be executed by the owner or manager
/// Requests a redelegation from source validator to target validator
pub fn execute_redelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    source_val: String,
    target_val: String,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?.assert_initialized()?;
    let params: Parameters = PARAMETERS.load(deps.storage)?;

    let source_val_addr = Addr::unchecked(&source_val);
    let target_val_addr = Addr::unchecked(&target_val);

    if info.sender != config.owner && info.sender != config.manager {
        return Err(StdError::generic_err("unauthorized"));
    }

    let is_source_valid = is_valid_validator(deps.storage, &source_val_addr)?;
    let is_target_valid = is_valid_validator(deps.storage, &target_val_addr)?;

    if !is_source_valid || !is_target_valid {
        return Err(StdError::generic_err("Invalid validators"));
    }

    let del_query_res = deps
        .querier
        .query_delegation(env.contract.address.clone(), source_val_addr.clone())?;

    if let Some(delegation) = del_query_res {
        if delegation.amount.amount.lt(&amount) {
            return Err(StdError::generic_err(
                "The delegation of the source validator is less than the requested amount",
            ));
        }

        if delegation
            .can_redelegate
            .amount
            .ne(&delegation.amount.amount)
        {
            return Err(StdError::generic_err("There is a redelegation in progress"));
        }
    } else {
        return Err(StdError::generic_err(
            "The source validator delegation is not available",
        ));
    };

    let messages: Vec<SubMsg> = vec![
        SubMsg::new(CosmosMsg::Staking(StakingMsg::Redelegate {
            src_validator: source_val_addr.to_string(),
            dst_validator: target_val_addr.to_string(),
            amount: Coin::new(amount.u128(), params.underlying_coin_denom),
        })),
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::UpdateGlobalIndex {
                airdrop_hooks: None,
            })?,
            funds: vec![],
        })),
    ];

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "redelegate"),
            attr("source_validator", source_val),
            attr("target_validator", target_val),
            attr("redelegated_amount", amount),
        ]))
}
