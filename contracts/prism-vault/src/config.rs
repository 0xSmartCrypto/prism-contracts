use crate::{
    contract::validate_rate,
    state::{
        read_validators, remove_white_validators, store_white_validators, Parameters, CONFIG,
        PARAMETERS,
    },
};
use cosmwasm_std::{
    attr, to_binary, Addr, Attribute, CosmosMsg, Decimal, DepsMut, DistributionMsg, Env,
    MessageInfo, Reply, ReplyOn, Response, StakingMsg, StdError, StdResult, SubMsg, WasmMsg,
};
use cw20::MinterResponse;
use prism_protocol::{internal::parse_reply_instantiate_data, vault::ExecuteMsg};
use prismswap::token::InstantiateMsg as TokenInstantiateMsg;

use rand::{Rng, SeedableRng, XorShiftRng};

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
    yluna_staking: Option<String>,
    airdrop_registry_contract: Option<String>,
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

    if let Some(yluna_staking) = yluna_staking {
        config.yluna_staking = deps.api.addr_validate(&yluna_staking)?;

        // register the reward contract for automate reward withdrawal.
        messages.push(SubMsg::new(CosmosMsg::Distribution(
            DistributionMsg::SetWithdrawAddress {
                address: yluna_staking,
            },
        )));
    }

    if let Some(airdrop) = airdrop_registry_contract {
        config.airdrop_registry_contract = deps.api.addr_validate(&airdrop)?;
    }

    let placeholder_addr = Addr::unchecked("");
    if !config.initialized
        && config.yluna_staking.ne(&placeholder_addr)
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
    let vault_conf = CONFIG.load(deps.storage)?.assert_initialized()?;

    if vault_conf.owner != info.sender && env.contract.address != info.sender {
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
) -> StdResult<Response> {
    let token = CONFIG.load(deps.storage)?.assert_initialized()?;
    let validator_addr = Addr::unchecked(&validator);

    if token.owner != info.sender {
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

    let mut replaced_val = Addr::unchecked("");
    let mut messages: Vec<SubMsg> = vec![];

    if let Ok(q) = query {
        let delegated_amount = q;
        let validators = read_validators(deps.storage)?;

        // redelegate the amount to a random validator.
        let block_height = env.block.height;
        let mut rng = XorShiftRng::seed_from_u64(block_height);
        let random_index = rng.gen_range(0, validators.len());
        replaced_val = Addr::unchecked(validators.get(random_index).unwrap().as_str());

        if let Some(delegation) = delegated_amount {
            messages.push(SubMsg::new(CosmosMsg::Staking(StakingMsg::Redelegate {
                src_validator: validator.to_string(),
                dst_validator: replaced_val.to_string(),
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
            attr("new-validator", replaced_val),
        ]))
}
