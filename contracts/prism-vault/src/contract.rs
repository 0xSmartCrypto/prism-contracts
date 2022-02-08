#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut,
    DistributionMsg, Env, MessageInfo, QueryRequest, Reply, ReplyOn, Response, StakingMsg,
    StdError, StdResult, SubMsg, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;

use crate::config::{
    execute_deregister_validator, execute_register_validator, execute_update_config,
    execute_update_params, set_token_address,
};

use crate::state::{
    all_unbond_history, get_unbond_requests, query_get_finished_amount, read_valid_validators,
    Config, CurrentBatch, Parameters, State, CONFIG, CURRENT_BATCH, PARAMETERS, STATE,
};
use crate::unbond::{execute_unbond, execute_withdraw_unbonded};

use crate::bond::{execute_bond, execute_bond_split};
use crate::refract::{merge, split};
use cw0::must_pay;
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse, TokenInfoResponse};
use cw_asset::{Asset, AssetInfo};
use prism_protocol::vault::{
    AllHistoryResponse, ConfigResponse, CurrentBatchResponse, Cw20HookMsg, ExecuteMsg,
    InstantiateMsg, QueryMsg, StateResponse, UnbondRequestsResponse, WhitelistedValidatorsResponse,
    WithdrawableUnbondedResponse,
};
use prism_protocol::yasset_staking::ExecuteMsg as StakingExecuteMsg;
use prismswap::querier::query_token_balance;
use prismswap::token::InstantiateMsg as TokenInstantiateMsg;

const CONTRACT_NAME: &str = "prism-vault";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let sender = info.sender.clone();

    let payment_amt = must_pay(&info, &msg.underlying_coin_denom)
        .map_err(|error| StdError::generic_err(format!("{}", error)))?;

    // store config
    // set placeholder addresses
    let config = Config {
        owner: sender,
        yluna_staking: Addr::unchecked(""),
        cluna_contract: Addr::unchecked(""),
        yluna_contract: Addr::unchecked(""),
        airdrop_registry_contract: Addr::unchecked(""),
        pluna_contract: Addr::unchecked(""),
        initialized: false, // will be set to true once yluna_staking and airdrop registry addresses are set
        token_code_id: msg.token_code_id,
        token_admin: deps.api.addr_validate(&msg.token_admin)?,
    };
    CONFIG.save(deps.storage, &config)?;

    // store state
    let state = State {
        exchange_rate: Decimal::one(),
        last_index_modification: env.block.time.seconds(),
        last_unbonded_time: env.block.time.seconds(),
        last_processed_batch: 0u64,
        total_bond_amount: payment_amt,
        ..Default::default()
    };

    STATE.save(deps.storage, &state)?;

    // instantiate parameters
    let params = Parameters {
        epoch_period: msg.epoch_period,
        underlying_coin_denom: msg.underlying_coin_denom.clone(),
        unbonding_period: msg.unbonding_period,
        peg_recovery_fee: validate_rate(msg.peg_recovery_fee)?,
        er_threshold: validate_rate(msg.er_threshold)?,
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
        amount: Coin {
            denom: msg.underlying_coin_denom,
            amount: payment_amt,
        },
    })));

    // start initialization of 3 tokens, cluna -> pluna -> yluna
    messages.push(SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: config.token_code_id,
            msg: to_binary(&TokenInstantiateMsg {
                name: "Prism cLuna Token".to_string(),
                symbol: "cLuna".to_string(),
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
    });

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("register-validator", msg.validator),
            attr("bond", payment_amt),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Bond { validator } => execute_bond(deps, env, info, validator),
        ExecuteMsg::BondSplit { validator } => execute_bond_split(deps, env, info, validator),
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
            airdrop_registry_contract,
        } => execute_update_config(deps, info, owner, yluna_staking, airdrop_registry_contract),
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
    let contract_addr = info.sender;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Unbond {}) => {
            // only token contract can execute this message
            let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
            if contract_addr != conf.cluna_contract {
                return Err(StdError::generic_err("unauthorized"));
            }
            execute_unbond(deps, env, cw20_msg.amount, cw20_msg.sender)
        }
        Err(err) => Err(err),
    }
}

/// Replies received after token instantiation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    set_token_address(deps, env, msg)
}

/// Update general parameters
/// Permissionless
pub fn execute_update_global(
    deps: DepsMut,
    env: Env,
    airdrop_hooks: Option<Vec<Binary>>,
) -> StdResult<Response> {
    let mut messages: Vec<SubMsg> = vec![];
    let config = CONFIG.load(deps.storage)?.assert_initialized()?;

    if let Some(hooks) = airdrop_hooks {
        for msg in hooks {
            messages.push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.airdrop_registry_contract.to_string(),
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
        contract_addr: config.yluna_staking.to_string(),
        msg: to_binary(&StakingExecuteMsg::ProcessDelegatorRewards {}).unwrap(),
        funds: vec![],
    })));

    // update state last modified
    STATE.update(deps.storage, |mut last_state| -> StdResult<State> {
        last_state.last_index_modification = env.block.time.seconds();
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
pub fn slashing(
    deps: &mut DepsMut,
    env: Env,
    state: &mut State,
    params: &Parameters,
) -> StdResult<()> {
    // Check the actual bonded amount
    let delegations = deps.querier.query_all_delegations(env.contract.address)?;
    if delegations.is_empty() {
        Ok(())
    } else {
        let mut actual_total_bonded = Uint128::zero();
        for delegation in delegations {
            if delegation.amount.denom == params.underlying_coin_denom {
                actual_total_bonded += delegation.amount.amount
            }
        }

        // Slashing happens if the expected amount is less than stored amount
        if state.total_bond_amount > actual_total_bonded {
            // Need total issued for updating the exchange rate
            let total_issued = query_total_issued(deps.as_ref())?;
            let current_requested_fee = CURRENT_BATCH.load(deps.storage)?.requested_with_fee;
            state.total_bond_amount = actual_total_bonded;
            state.update_exchange_rate(total_issued, current_requested_fee);
            STATE.save(deps.storage, state)?;
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;

    if info.sender != conf.airdrop_registry_contract {
        return Err(StdError::generic_err("unauthorized"));
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
    let conf = CONFIG.load(deps.storage)?.assert_initialized()?;
    let airdrop_token_addr = deps.api.addr_validate(&airdrop_token_contract)?;

    let amount = query_token_balance(&deps.querier, &airdrop_token_addr, &env.contract.address)?;

    let airdrop_reward = Asset {
        info: AssetInfo::Cw20(airdrop_token_addr.clone()),
        amount,
    };

    Ok(Response::new().add_messages(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: airdrop_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: conf.yluna_staking.to_string(),
                amount,
                expires: None,
            })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: conf.yluna_staking.to_string(),
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
    let params = PARAMETERS.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    slashing(&mut deps, env, &mut state, &params)?;
    // read state for log
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
        QueryMsg::UnbondRequests {
            address,
            start_from,
            limit,
        } => to_binary(&query_unbond_requests(deps, address, start_from, limit)?),
        QueryMsg::AllHistory { start_from, limit } => {
            to_binary(&query_unbond_requests_limitation(deps, start_from, limit)?)
        }
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

fn query_white_validators(deps: Deps) -> StdResult<WhitelistedValidatorsResponse> {
    let validators = read_valid_validators(deps.storage)?
        .iter()
        .map(|item| item.to_string())
        .collect();
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
    let addr = deps.api.addr_validate(&address)?;
    // query the finished amount with the default limit (None), to obtain the same value as the result of the actual unbond operation
    let all_requests = query_get_finished_amount(deps.storage, &addr, historical_time, None)?;

    let withdrawable = WithdrawableUnbondedResponse {
        withdrawable: all_requests,
    };
    Ok(withdrawable)
}

fn query_params(deps: Deps) -> StdResult<Parameters> {
    PARAMETERS.load(deps.storage)
}

pub(crate) fn query_total_issued(deps: Deps) -> StdResult<Uint128> {
    let cfg = CONFIG.load(deps.storage)?;

    let cluna_token_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: cfg.cluna_contract.to_string(),
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
        }))?;

    // query pLuna and yLuna supply and use the minimum of the two values
    let pluna_token_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: cfg.pluna_contract.to_string(),
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
        }))?;
    let yluna_token_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: cfg.yluna_contract.to_string(),
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
        }))?;

    Ok(cluna_token_info.total_supply
        + pluna_token_info
            .total_supply
            .min(yluna_token_info.total_supply))
}

fn query_unbond_requests(
    deps: Deps,
    address: String,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<UnbondRequestsResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let requests = get_unbond_requests(deps.storage, &addr, start, limit)?;
    let res = UnbondRequestsResponse { address, requests };
    Ok(res)
}

fn query_unbond_requests_limitation(
    deps: Deps,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<AllHistoryResponse> {
    let requests = all_unbond_history(deps.storage, start, limit)?;
    let requests_res = requests.iter().map(|item| item.as_res()).collect();
    let res = AllHistoryResponse {
        history: requests_res,
    };
    Ok(res)
}

pub fn validate_rate(rate: Decimal) -> StdResult<Decimal> {
    if rate > Decimal::one() {
        return Err(StdError::generic_err(format!(
            "Rate can not be bigger than one (given value: {})",
            rate
        )));
    }

    Ok(rate)
}
