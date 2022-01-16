#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, Decimal, Deps, DepsMut, Env, Response, Uint128, WasmMsg,
};

use prism_common::decimal_division;

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroGenExecuteMsg};
use astroport::pair::Cw20HookMsg as AstroPairHookMsg;

use crate::error::{ContractError, ContractResult};
use crate::query::{query_lp_burn_rewards, query_pending_generator_rewards, query_pool_info};

use crate::state::{CONFIG, LP_INFO};
use cw20::Cw20ExecuteMsg;

// takes in amount of LP to bond
pub fn bond(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> ContractResult<Response> {
    if amount <= Uint128::zero() {
        return Err(ContractError::BadBondAmount {});
    }
    let config = CONFIG.load(deps.storage)?;

    // update rewards
    let (liquidity, lp_to_burn, mut messages) = update_rewards(deps.as_ref(), env)?;

    // attempt to send LP to astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: staking_token.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: config.generator.into_string(),
            msg: to_binary(&AstroHookMsg::Deposit {})?,
            amount,
        })?,
        funds: vec![],
    }));

    // save internal state and calculate tokens to mint
    // rounding issues?
    let mut lp_info = LP_INFO.load(deps.storage)?;
    lp_info.last_liquidity = liquidity;
    lp_info.amt_lp = lp_info.amt_lp.checked_sub(lp_to_burn)?;
    let mut clp_to_mint = amount;
    if lp_info.amt_lp > Uint128::zero() {
        clp_to_mint *= lp_info.amt_clp / lp_info.amt_lp;
    }
    lp_info.amt_lp += amount;
    lp_info.amt_clp += clp_to_mint;
    LP_INFO.save(deps.storage, &lp_info)?;

    // mint cLP tokens
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: sender_addr.to_string(),
            amount: clp_to_mint,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "bond"),
        attr("from", sender_addr.as_str()),
        attr("LP", staking_token.as_str()),
        attr("amount", amount),
    ]))
}

// takes in amount of LP to unbond, not cLP
pub fn unbond(
    deps: DepsMut,
    env: Env,
    sender_addr: Addr,
    amount: Uint128,
) -> ContractResult<Response> {
    if amount <= Uint128::zero() {
        return Err(ContractError::BadUnbondAmount {});
    }

    let config = CONFIG.load(deps.storage)?;
    let lp_info = LP_INFO.load(deps.storage)?;
    let lp_contract = lp_info.lp_contract;

    // update rewards
    let (liquidity, lp_to_burn, mut messages) = update_rewards(deps.as_ref(), env)?;

    // attempt to withdraw LP from astro generator
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator.into_string(),
        msg: to_binary(&AstroGenExecuteMsg::Withdraw {
            lp_token: lp_contract.clone(),
            amount,
        })?,
        funds: vec![],
    }));

    // save internal state and calculate tokens to burn
    // rounding issues?
    let mut lp_info = LP_INFO.load(deps.storage)?;
    lp_info.last_liquidity = liquidity;
    lp_info.amt_lp = lp_info.amt_lp.checked_sub(lp_to_burn)?;
    let mut clp_to_burn = amount;
    if lp_info.amt_lp > Uint128::zero() {
        clp_to_burn *= lp_info.amt_clp / lp_info.amt_lp;
    }
    lp_info.amt_lp = lp_info.amt_lp.checked_sub(amount)?;
    lp_info.amt_clp = lp_info.amt_clp.checked_sub(clp_to_burn)?;
    LP_INFO.save(deps.storage, &lp_info)?;

    // burn cLP tokens from user
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.clp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: sender_addr.to_string(),
            amount: clp_to_burn,
        })?,
        funds: vec![],
    }));

    // transfer LP to user
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_contract.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: sender_addr.to_string(),
            amount,
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "unbond"),
        attr("from", sender_addr.as_str()),
        attr("LP", lp_contract.as_str()),
        attr("amount", amount),
    ]))
}

// generates at most 6 messages
pub fn update_rewards(deps: Deps, env: Env) -> ContractResult<(Decimal, Uint128, Vec<CosmosMsg>)> {
    let config = CONFIG.load(deps.storage)?;
    let lp_info = LP_INFO.load(deps.storage)?;
    let token = lp_info.lp_contract.clone();

    // calculate and amount of LP to withdraw and burn
    let pool_info = query_pool_info(deps, &deps.querier)?;

    // why is math so hard to do between Decimal and Uint128
    // s = liquidity per token = sqrt(xy)/number of LP
    // withdraw and burn (1 - s_last/s_new)*vault_share of LP tokens
    let s = Decimal::from_ratio(
        pool_info.assets[0].amount * pool_info.assets[1].amount,
        Uint128::new(1),
    )
    .sqrt();
    let new_liquidity: Decimal = s / pool_info.total_share;
    let inv_new_liquidity = decimal_division(Uint128::new(1), new_liquidity);
    let inv_last_liquidity = decimal_division(Uint128::new(1), lp_info.last_liquidity);
    let lp_to_burn =
        (Uint128::new(1).checked_sub(inv_new_liquidity / inv_last_liquidity)?) * lp_info.amt_lp;

    // query generator reward
    let pending_gen_rewards = query_pending_generator_rewards(deps, env, &deps.querier)?;

    // query amm reward
    let pending_amm_rewards = query_lp_burn_rewards(deps, &deps.querier, lp_to_burn)?;

    // claim generator reward, withdrawn and burn LP
    let mut messages = vec![];
    if lp_to_burn > Uint128::zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.generator.clone().into_string(),
            msg: to_binary(&AstroGenExecuteMsg::Withdraw {
                lp_token: token.clone(),
                amount: lp_to_burn,
            })?,
            funds: vec![],
        }));
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token.into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: lp_info.pair_contract.into_string(),
                msg: to_binary(&AstroPairHookMsg::WithdrawLiquidity {})?,
                amount: lp_to_burn,
            })?,
            funds: vec![],
        }));
    }

    // send rewards to reward-dist
    for reward in pending_gen_rewards {
        if reward.amount > Uint128::zero() {
            messages.push(reward.into_msg(&deps.querier, config.reward_dist.clone())?);
        }
    }
    for reward in pending_amm_rewards {
        if reward.amount > Uint128::zero() {
            messages.push(reward.into_msg(&deps.querier, config.reward_dist.clone())?);
        }
    }

    Ok((new_liquidity, lp_to_burn, messages))
}
