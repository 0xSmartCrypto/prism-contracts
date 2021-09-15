#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, DistributionMsg,
    Env, MessageInfo, QueryRequest, Response, StakingMsg, StdError, StdResult, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};

use crate::config::{
    execute_deregister_validator, execute_register_validator, execute_update_config,
    execute_update_params,
};

use crate::state::{
    all_unbond_history, get_unbond_requests, query_get_finished_amount, read_valid_validators,
    CurrentBatch, Parameters, CONFIG, CURRENT_BATCH, PARAMETERS, STATE,
};
use crate::unbond::{execute_unbond, execute_withdraw_unbonded};

use crate::bond::execute_bond;
use crate::refract::{merge, split};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw20_legacy::state::TokenInfo;
use prism_protocol::vault::{
    AllHistoryResponse, Config, ConfigResponse, CurrentBatchResponse, Cw20HookMsg, ExecuteMsg,
    InstantiateMsg, MigrateMsg, QueryMsg, State, StateResponse, UnbondRequestsResponse,
    WhitelistedValidatorsResponse, WithdrawableUnbondedResponse,
};
use prism_protocol::yasset_staking::ExecuteMsg as StakingExecuteMsg;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::query_token_balance;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let sender = info.sender.clone();

    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == msg.underlying_coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", "uluna"))
        })?;

    // store config
    // TODO -- auto create yluna, pluna, cluna token contracts from token code id
    let data = Config {
        creator: sender.to_string(),
        yluna_staking: None,
        cluna_contract: None,
        yluna_contract: None,
        airdrop_registry_contract: None,
        pluna_contract: None,
    };
    CONFIG.save(deps.storage, &data)?;

    // store state
    let state = State {
        exchange_rate: Decimal::one(),
        last_index_modification: env.block.time.nanos(),
        last_unbonded_time: env.block.time.nanos(),
        last_processed_batch: 0u64,
        total_bond_amount: payment.amount,
        ..Default::default()
    };

    STATE.save(deps.storage, &state)?;

    // instantiate parameters
    let params = Parameters {
        epoch_period: msg.epoch_period,
        underlying_coin_denom: msg.underlying_coin_denom,
        unbonding_period: msg.unbonding_period,
        peg_recovery_fee: msg.peg_recovery_fee,
        er_threshold: msg.er_threshold,
    };

    PARAMETERS.save(deps.storage, &params)?;

    let batch = CurrentBatch {
        id: 1,
        requested_with_fee: Default::default(),
    };
    CURRENT_BATCH.save(deps.storage, &batch)?;

    let mut messages = vec![];

    // register the given validator
    let register_validator = ExecuteMsg::RegisterValidator {
        validator: msg.validator.clone(),
    };
    messages.push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&register_validator).unwrap(),
        funds: vec![],
    })));

    // send the delegate message
    messages.push(SubMsg::new(CosmosMsg::Staking(StakingMsg::Delegate {
        validator: msg.validator.to_string(),
        amount: payment.clone(),
    })));

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("register-validator", msg.validator),
            attr("bond", payment.amount),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Bond { validator } => execute_bond(deps, env, info, validator),
        ExecuteMsg::UpdateGlobalIndex { airdrop_hooks } => {
            execute_update_global(deps, env, airdrop_hooks)
        }
        ExecuteMsg::WithdrawUnbonded {} => execute_withdraw_unbonded(deps, env, info),
        ExecuteMsg::RegisterValidator { validator } => {
            execute_register_validator(deps, env, info, validator)
        }
        ExecuteMsg::DeregisterValidator { validator } => {
            execute_deregister_validator(deps, env, info, validator)
        }
        ExecuteMsg::CheckSlashing {} => execute_slashing(deps, env),
        ExecuteMsg::UpdateParams {
            epoch_period,
            unbonding_period,
            peg_recovery_fee,
            er_threshold,
        } => execute_update_params(
            deps,
            env,
            info,
            epoch_period,
            unbonding_period,
            peg_recovery_fee,
            er_threshold,
        ),
        ExecuteMsg::UpdateConfig {
            owner,
            yluna_staking,
            cluna_contract,
            yluna_contract,
            pluna_contract,
            airdrop_registry_contract,
        } => execute_update_config(
            deps,
            env,
            info,
            owner,
            yluna_staking,
            cluna_contract,
            yluna_contract,
            pluna_contract,
            airdrop_registry_contract,
        ),
        ExecuteMsg::ClaimAirdrop {
            airdrop_token_contract,
            airdrop_contract,
            claim_msg,
        } => claim_airdrop(
            deps,
            env,
            info,
            airdrop_token_contract,
            airdrop_contract,
            claim_msg,
        ),
        ExecuteMsg::Split { amount } => split(deps, info, amount),
        ExecuteMsg::Merge { amount } => merge(deps, info, amount),
        ExecuteMsg::DepositAirdropReward {
            airdrop_token_contract,
        } => deposit_airdrop_rewards(deps, env, airdrop_token_contract),
    }
}

/// CW20 token receive handler.
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let contract_addr = info.sender.clone();

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Unbond {}) => {
            // only token contract can execute this message
            let conf = CONFIG.load(deps.storage)?;
            if contract_addr
                != conf
                    .cluna_contract
                    .expect("the token contract must have been registered")
            {
                return Err(StdError::generic_err("unauthorized"));
            }
            execute_unbond(deps, env, info, cw20_msg.amount, cw20_msg.sender)
        }
        Err(err) => Err(err),
    }
}

/// Update general parameters
/// Permissionless
pub fn execute_update_global(
    deps: DepsMut,
    env: Env,
    airdrop_hooks: Option<Vec<Binary>>,
) -> StdResult<Response> {
    let mut messages: Vec<SubMsg> = vec![];

    let config = CONFIG.load(deps.storage)?;
    let yluna_staking_addr = config
        .yluna_staking
        .expect("the reward contract must have been registered");

    if airdrop_hooks.is_some() {
        let registry_addr = config.airdrop_registry_contract.unwrap();
        for msg in airdrop_hooks.unwrap() {
            messages.push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: registry_addr.clone(),
                msg,
                funds: vec![],
            })))
        }
    }

    // Send withdraw message
    let mut withdraw_msgs = withdraw_all_rewards(&deps, env.contract.address.clone())?;
    messages.append(&mut withdraw_msgs);

    // Swap to $UST, then into $PRISM
    messages.push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: yluna_staking_addr.clone(),
        msg: to_binary(&StakingExecuteMsg::ProcessDelegatorRewards {}).unwrap(),
        funds: vec![],
    })));

    //update state last modified
    STATE.update(deps.storage, |mut last_state| -> StdResult<State> {
        last_state.last_index_modification = env.block.time.nanos();
        Ok(last_state)
    })?;

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![attr("action", "update_global_index")]))
}

/// Create withdraw requests for all validators
fn withdraw_all_rewards(deps: &DepsMut, delegator: Addr) -> StdResult<Vec<SubMsg>> {
    let mut messages: Vec<SubMsg> = vec![];
    let delegations = deps.querier.query_all_delegations(delegator);

    if let Ok(delegations) = delegations {
        for delegation in delegations {
            let msg: CosmosMsg =
                CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
                    validator: delegation.validator,
                });
            messages.push(SubMsg::new(msg));
        }
    }

    Ok(messages)
}

/// Check whether slashing has happened
/// This is used for checking slashing while bonding or unbonding
pub fn slashing(deps: &mut DepsMut, env: Env) -> StdResult<()> {
    //read params
    let params = PARAMETERS.load(deps.storage)?;
    let coin_denom = params.underlying_coin_denom;

    // Check the amount that contract thinks is bonded
    let state_total_bonded = STATE.load(deps.storage)?.total_bond_amount;

    // Check the actual bonded amount
    let delegations = deps.querier.query_all_delegations(env.contract.address)?;
    if delegations.is_empty() {
        Ok(())
    } else {
        let mut actual_total_bonded = Uint128::zero();
        for delegation in delegations {
            if delegation.amount.denom == coin_denom {
                actual_total_bonded += delegation.amount.amount
            }
        }

        // Need total issued for updating the exchange rate
        let total_issued = query_total_issued(deps.as_ref())?;
        let current_requested_fee = CURRENT_BATCH.load(deps.storage)?.requested_with_fee;

        // Slashing happens if the expected amount is less than stored amount
        if state_total_bonded.u128() > actual_total_bonded.u128() {
            STATE.update(deps.storage, |mut state| -> StdResult<State> {
                state.total_bond_amount = actual_total_bonded;
                state.update_exchange_rate(total_issued, current_requested_fee);
                Ok(state)
            })?;
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn claim_airdrop(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    airdrop_token_contract: String,
    airdrop_contract: String,
    claim_msg: Binary,
) -> StdResult<Response> {
    let conf = CONFIG.load(deps.storage)?;

    let airdrop_reg = conf.airdrop_registry_contract.unwrap();

    if info.sender.to_string() != airdrop_reg {
        return Err(StdError::generic_err(format!(
            "Sender must be {}",
            airdrop_reg
        )));
    }

    let mut messages: Vec<SubMsg> = vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: airdrop_contract,
        msg: claim_msg,
        funds: vec![],
    }))];

    messages.push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::DepositAirdropReward {
            airdrop_token_contract,
        })?,
        funds: vec![],
    })));

    Ok(Response::new().add_submessages(messages))
}

pub fn deposit_airdrop_rewards(
    deps: DepsMut,
    env: Env,
    airdrop_token_contract: String,
) -> StdResult<Response> {
    let conf = CONFIG.load(deps.storage)?;
    let yluna_staking_addr = conf
        .yluna_staking
        .expect("the reward contract must have been registered");

    let amount = query_token_balance(
        &deps.querier,
        Addr::unchecked(airdrop_token_contract.clone()),
        env.contract.address,
    )?;

    let airdrop_reward = Asset {
        info: AssetInfo::Token {
            contract_addr: airdrop_token_contract.clone(),
        },
        amount,
    };

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: airdrop_token_contract.clone(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: yluna_staking_addr.clone(),
                amount,
                expires: None,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: yluna_staking_addr.clone(),
            msg: to_binary(&StakingExecuteMsg::DepositRewards {
                assets: vec![airdrop_reward],
            })?,
            funds: vec![],
        }),
    ]))
}

/// Handler for tracking slashing
pub fn execute_slashing(mut deps: DepsMut, env: Env) -> StdResult<Response> {
    // call slashing
    slashing(&mut deps, env)?;
    // read state for log
    let state = STATE.load(deps.storage)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "check_slashing"),
        attr("new_exchange_rate", state.exchange_rate.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::CurrentBatch {} => to_binary(&query_current_batch(deps)?),
        QueryMsg::WhitelistedValidators {} => to_binary(&query_white_validators(deps)?),
        QueryMsg::WithdrawableUnbonded { address } => {
            to_binary(&query_withdrawable_unbonded(deps, address, env)?)
        }
        QueryMsg::Parameters {} => to_binary(&query_params(deps)?),
        QueryMsg::UnbondRequests { address } => to_binary(&query_unbond_requests(deps, address)?),
        QueryMsg::AllHistory { start_from, limit } => {
            to_binary(&query_unbond_requests_limitation(deps, start_from, limit)?)
        }
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.creator,
        yluna_staking: config.yluna_staking,
        cluna_contract: config.cluna_contract,
        pluna_contract: config.pluna_contract,
        yluna_contract: config.yluna_contract,
        airdrop_registry_contract: config.airdrop_registry_contract,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    let res = StateResponse {
        exchange_rate: state.exchange_rate,
        total_bond_amount: state.total_bond_amount,
        last_index_modification: state.last_index_modification,
        prev_vault_balance: state.prev_vault_balance,
        actual_unbonded_amount: state.actual_unbonded_amount,
        last_unbonded_time: state.last_unbonded_time,
        last_processed_batch: state.last_processed_batch,
    };
    Ok(res)
}

fn query_white_validators(deps: Deps) -> StdResult<WhitelistedValidatorsResponse> {
    let validators = read_valid_validators(deps.storage)?;
    let response = WhitelistedValidatorsResponse { validators };
    Ok(response)
}

fn query_current_batch(deps: Deps) -> StdResult<CurrentBatchResponse> {
    let current_batch = CURRENT_BATCH.load(deps.storage)?;
    Ok(CurrentBatchResponse {
        id: current_batch.id,
        requested_with_fee: current_batch.requested_with_fee,
    })
}

fn query_withdrawable_unbonded(
    deps: Deps,
    address: String,
    env: Env,
) -> StdResult<WithdrawableUnbondedResponse> {
    let params = PARAMETERS.load(deps.storage)?;
    let historical_time = env.block.time.seconds() - params.unbonding_period;
    let all_requests = query_get_finished_amount(deps.storage, address, historical_time)?;

    let withdrawable = WithdrawableUnbondedResponse {
        withdrawable: all_requests,
    };
    Ok(withdrawable)
}

fn query_params(deps: Deps) -> StdResult<Parameters> {
    PARAMETERS.load(deps.storage)
}

pub(crate) fn query_total_issued(deps: Deps) -> StdResult<Uint128> {
    let token_address = CONFIG
        .load(deps.storage)?
        .cluna_contract
        .expect("token contract must have been registered");
    let token_info: TokenInfo = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: token_address,
        key: Binary::from("token_info".as_bytes()),
    }))?;
    Ok(token_info.total_supply)
}

fn query_unbond_requests(deps: Deps, address: String) -> StdResult<UnbondRequestsResponse> {
    let requests = get_unbond_requests(deps.storage, address.clone())?;
    let res = UnbondRequestsResponse { address, requests };
    Ok(res)
}

fn query_unbond_requests_limitation(
    deps: Deps,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<AllHistoryResponse> {
    let requests = all_unbond_history(deps.storage, start, limit)?;
    let res = AllHistoryResponse { history: requests };
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
