use crate::contract::{query_total_issued, slashing};
use crate::math::decimal_division;
use crate::state::{
    is_valid_validator, read_valid_validators, CONFIG, CURRENT_BATCH, PARAMETERS, STATE,
};
use cosmwasm_std::{
    attr, to_binary, CosmosMsg, DepsMut, Env, MessageInfo, Response, StakingMsg, StdError,
    StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw20_base::msg::ExecuteMsg as TokenMsg;
use prism_protocol::vault::State;

pub fn execute_bond_split(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator: Option<String>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let params = PARAMETERS.load(deps.storage)?;
    let coin_denom = params.underlying_coin_denom;

    let (mint_amount_with_fee, mut sub_messages) = _execute_bond(deps, &env, &info, &validator)?;
    sub_messages.pop();

    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", coin_denom))
        })?;

    Ok(Response::new()
        .add_submessages(sub_messages)
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.yluna_contract.unwrap(),
                msg: to_binary(&TokenMsg::Mint {
                    recipient: info.sender.clone().into_string(),
                    amount: mint_amount_with_fee.clone(),
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.pluna_contract.unwrap(),
                msg: to_binary(&TokenMsg::Mint {
                    recipient: info.sender.clone().into_string(),
                    amount: mint_amount_with_fee.clone(),
                })?,
                funds: vec![],
            }),
        ])
        .add_attributes(vec![
            attr("action", "bond_split"),
            attr("from", info.sender.as_str().clone()),
            attr("bonded", payment.amount),
            attr("minted", mint_amount_with_fee),
        ]))
}

pub fn _execute_bond(
    mut deps: DepsMut,
    env: &Env,
    info: &MessageInfo,
    validator: &Option<String>,
) -> StdResult<(Uint128, Vec<SubMsg>)> {
    // validator must be whitelisted

    let unwrapped_validator = match validator {
        Some(v) => deps.api.addr_validate(v)?,
        None => {
            let validators = read_valid_validators(deps.storage)?;
            let idx = env.block.time.nanos() as usize % validators.len();
            validators[idx].clone()
        }
    };
    let is_valid = is_valid_validator(deps.storage, &unwrapped_validator)?;
    if !is_valid {
        return Err(StdError::generic_err(
            "The chosen validator is currently not supported",
        ));
    }

    let params = PARAMETERS.load(deps.storage)?;
    let coin_denom = params.underlying_coin_denom;
    let threshold = params.er_threshold;
    let recovery_fee = params.peg_recovery_fee;

    // current batch requested fee is need for accurate exchange rate computation.
    let current_batch = CURRENT_BATCH.load(deps.storage)?;
    let requested_with_fee = current_batch.requested_with_fee;

    // coin must have be sent along with transaction and it should be in underlying coin denom
    if info.funds.len() > 1usize {
        return Err(StdError::generic_err(
            "More than one coin is sent; only one asset is supported",
        ));
    }

    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", coin_denom))
        })?;

    // check slashing
    slashing(&mut deps, env.clone())?;

    let state = STATE.load(deps.storage)?;
    let sender = info.sender.clone();

    // get the total supply
    let mut total_supply = query_total_issued(deps.as_ref()).unwrap_or_default();

    // peg recovery fee should be considered
    let mint_amount = decimal_division(payment.amount, state.exchange_rate);
    let mut mint_amount_with_fee = mint_amount;
    if state.exchange_rate < threshold {
        let max_peg_fee = mint_amount * recovery_fee;
        let required_peg_fee = ((total_supply + mint_amount + current_batch.requested_with_fee)
            .checked_sub(state.total_bond_amount + payment.amount))?;
        let peg_fee = Uint128::min(max_peg_fee, required_peg_fee);
        mint_amount_with_fee = (mint_amount.checked_sub(peg_fee))?;
    }

    // total supply should be updated for exchange rate calculation.
    total_supply += mint_amount_with_fee;

    // exchange rate should be updated for future
    STATE.update(deps.storage, |mut prev_state| -> StdResult<State> {
        prev_state.total_bond_amount += payment.amount;
        prev_state.update_exchange_rate(total_supply, requested_with_fee);
        Ok(prev_state)
    })?;

    let config = CONFIG.load(deps.storage)?;
    let token_address = config
        .cluna_contract
        .expect("the cluna contract must have been registered");

    Ok((
        mint_amount_with_fee,
        vec![
            SubMsg::new(CosmosMsg::Staking(StakingMsg::Delegate {
                validator: unwrapped_validator.into_string(),
                amount: payment.clone(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: token_address,
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: sender.to_string(),
                    amount: mint_amount_with_fee,
                })?,
                funds: vec![],
            })),
        ],
    ))
}

pub fn execute_bond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator: Option<String>,
) -> StdResult<Response> {
    let params = PARAMETERS.load(deps.storage)?;
    let coin_denom = params.underlying_coin_denom;

    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == coin_denom && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} assets are provided to bond", coin_denom))
        })?;

    let (mint_amount_with_fee, messages) = _execute_bond(deps, &env, &info, &validator)?;
    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "bond"),
            attr("from", info.sender.as_str().clone()),
            attr("bonded", payment.amount),
            attr("minted", mint_amount_with_fee),
        ]))
}
