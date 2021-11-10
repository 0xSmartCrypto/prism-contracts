use crate::state::{
    read_validators, remove_white_validators, store_white_validators, Parameters, CONFIG,
    PARAMETERS,
};
use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, Decimal, DepsMut, DistributionMsg, Env, MessageInfo,
    Response, StakingMsg, StdError, StdResult, SubMsg, WasmMsg,
};
use prism_protocol::vault::{Config, ExecuteMsg};

use rand::{Rng, SeedableRng, XorShiftRng};

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

    if info.sender.as_str() != config.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    let params: Parameters = PARAMETERS.load(deps.storage)?;

    let new_params = Parameters {
        epoch_period: epoch_period.unwrap_or(params.epoch_period),
        underlying_coin_denom: params.underlying_coin_denom,
        unbonding_period: unbonding_period.unwrap_or(params.unbonding_period),
        peg_recovery_fee: peg_recovery_fee.unwrap_or(params.peg_recovery_fee),
        er_threshold: er_threshold.unwrap_or(params.er_threshold),
    };

    PARAMETERS.save(deps.storage, &new_params)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_params")]))
}

/// Update the config. Update the owner, reward and token contracts.
/// Only creator/owner is allowed to execute
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    yluna_staking: Option<String>,
    cluna_contract: Option<String>,
    yluna_contract: Option<String>,
    pluna_contract: Option<String>,
    airdrop_registry_contract: Option<String>,
) -> StdResult<Response> {
    // only owner must be able to send this message.
    let conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.creator {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut messages: Vec<SubMsg> = vec![];

    if let Some(o) = owner {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.creator = o;
            Ok(last_config)
        })?;
    }
    if let Some(reward) = yluna_staking {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.yluna_staking = Some(reward.clone());
            Ok(last_config)
        })?;

        // register the reward contract for automate reward withdrawal.
        messages.push(SubMsg::new(CosmosMsg::Distribution(
            DistributionMsg::SetWithdrawAddress { address: reward },
        )));
    }

    if let Some(token) = cluna_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.cluna_contract = Some(token);
            Ok(last_config)
        })?;
    }

    if let Some(token) = yluna_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.yluna_contract = Some(token);
            Ok(last_config)
        })?;
    }

    if let Some(token) = pluna_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.pluna_contract = Some(token);
            Ok(last_config)
        })?;
    }

    if let Some(airdrop) = airdrop_registry_contract {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.airdrop_registry_contract = Some(airdrop);
            Ok(last_config)
        })?;
    }

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![attr("action", "update_config")]))
}

/// Register a white listed validator.
/// Only creator/owner is allowed to execute
pub fn execute_register_validator(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator: String,
) -> StdResult<Response> {
    let vault_conf = CONFIG.load(deps.storage)?;

    if vault_conf.creator != info.sender.as_str()
        && env.contract.address.as_str() != info.sender.as_str()
    {
        return Err(StdError::generic_err("unauthorized"));
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

    store_white_validators(&mut deps, validator.clone())?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register_validator"),
        attr("validator", validator),
    ]))
}

/// Deregister a previously-whitelisted validator.
/// Only creator/owner is allowed to execute
pub fn execute_deregister_validator(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator: String,
) -> StdResult<Response> {
    let token = CONFIG.load(deps.storage)?;

    let validator_addr = deps.api.addr_validate(validator.as_str())?;
    if token.creator != info.sender.to_string() {
        return Err(StdError::generic_err("unauthorized"));
    }
    let validators_before_remove = read_validators(&deps.as_ref())?;

    if validators_before_remove.len() == 1 {
        return Err(StdError::generic_err(
            "Cannot remove the last whitelisted validator",
        ));
    }

    remove_white_validators(&mut deps, validator_addr.to_string())?;

    let query = deps
        .querier
        .query_delegation(env.contract.address.clone(), validator.clone());

    let mut replaced_val = Addr::unchecked("");
    let mut messages: Vec<SubMsg> = vec![];

    if let Ok(q) = query {
        let delegated_amount = q;
        let validators = read_validators(&deps.as_ref())?;

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
