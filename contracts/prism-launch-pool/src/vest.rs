use crate::contract::{_pull_pending_rewards, update_reward_indexes};
use crate::error::ContractError;
use crate::state::{Config, CONFIG, PENDING_WITHDRAW, REWARD_INFO, SCHEDULED_VEST};
use cosmwasm_std::Addr;
use cosmwasm_std::{
    to_binary, CosmosMsg, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult, Storage,
    Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_asset::{Asset, AssetInfo};
use cw_storage_plus::Bound;
use prism_protocol::gov::Cw20HookMsg as GovCw20HookMsg;
use prism_protocol::internal::de::deserialize_key;
use prism_protocol::launch_pool::{ClaimType, ExecuteMsg};
use prism_protocol::xprism_boost::Cw20HookMsg as BoostContractCw20HookMsg;
use prismswap::querier::query_token_balance;
use std::convert::TryInto;

// seconds in a day, make time discrete per day
pub const TIME_UNIT: u64 = 60 * 60 * 24;

// Cap the number of iterations when processing vested entries. Under normal
// conditions, with a daily bulk execution, for most users there should be a
// maximum of 30 entries.
pub const MAX_UPDATE_VEST_PER_TX: u64 = 50u64;

pub fn update_vest(storage: &mut dyn Storage, current_time: u64, address: &str) -> StdResult<()> {
    let mut can_withdraw = PENDING_WITHDRAW
        .load(storage, address.as_bytes())
        .unwrap_or_else(|_| Uint128::zero());
    let mut to_delete = vec![];

    for item in SCHEDULED_VEST
        .prefix(address.as_bytes())
        .range(storage, None, None, Order::Ascending)
        .take(MAX_UPDATE_VEST_PER_TX as usize)
    {
        let (key, unlocked) = item?;
        let end_time = u64::from_be_bytes(key.try_into().unwrap());
        if current_time < end_time {
            break;
        }
        can_withdraw += unlocked;
        to_delete.push(end_time);
    }

    for t in to_delete {
        SCHEDULED_VEST.remove(storage, (address.as_bytes(), &t.to_be_bytes()))
    }
    PENDING_WITHDRAW.save(storage, address.as_bytes(), &can_withdraw)
}

pub fn withdraw_rewards(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_reward_indexes(deps.storage, &env, &cfg)?;
    _withdraw_rewards_single(&mut deps, &env, &cfg, &info.sender)
}

fn _withdraw_rewards_single(
    deps: &mut DepsMut,
    env: &Env,
    cfg: &Config,
    human_address: &Addr,
) -> Result<Response, ContractError> {
    let mut reward_info = _pull_pending_rewards(deps.storage, human_address)?;

    let to_withdraw = reward_info.pending_reward;
    reward_info.pending_reward = Uint128::zero();
    REWARD_INFO.save(deps.storage, human_address.as_bytes(), &reward_info)?;

    update_vest(
        deps.storage,
        env.block.time.seconds(),
        human_address.as_str(),
    )?;

    if !to_withdraw.is_zero() {
        let mut end_time = env.block.time.seconds() + cfg.vesting_period;
        end_time -= end_time % TIME_UNIT;

        let orig_vest = SCHEDULED_VEST
            .load(
                deps.storage,
                (human_address.as_bytes(), &end_time.to_be_bytes()),
            )
            .unwrap_or_else(|_| Uint128::zero());
        SCHEDULED_VEST.save(
            deps.storage,
            (human_address.as_bytes(), &end_time.to_be_bytes()),
            &(orig_vest + to_withdraw),
        )?;
    }
    Ok(Response::new().add_attribute("withdraw_amount", to_withdraw.to_string()))
}

/// withdraw_rewards_bulk starts the vesting period for many accounts in a
/// single call. Specifically, this call processes a batch of up to `limit`
/// accounts sorted by increasing account address, starting at the first account
/// whose address is strictly greater than the given `start_after_address`.
///
///  This is intended to be called repeatedly with increasing values of
/// `start_after_address` to effectively paginate over all accounts.
///
/// If `start_after_address` is not provided, we'll start at the very first
/// address we know of.
///
/// Returns the last address processed in the batch to be used as
/// `start_after_address` on the next call, or an empty string if there are no
/// more addresses to process.
pub fn withdraw_rewards_bulk(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    limit: u64,
    start_after_address: Option<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.operator {
        return Err(ContractError::Unauthorized {});
    }

    update_reward_indexes(deps.storage, &env, &cfg)?;

    let start = match start_after_address {
        Some(address) => {
            deps.api.addr_validate(&address)?;
            Some(Bound::exclusive(address.as_bytes()))
        }
        None => None,
    };
    // Load all addresses in this batch in memory first, then iterate over them
    // and mutate things. This is to avoid mutating the REWARD_INFO map at the
    // same time that we are iterating over it (which I suspect could mess up
    // the iterator).
    let addresses: Vec<Addr> = REWARD_INFO
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|k| deserialize_key::<Addr>(k).unwrap())
        .collect();

    for address in &addresses {
        _withdraw_rewards_single(&mut deps, &env, &cfg, address)?;
    }

    // Return last address that was processed, for next call.
    let last_address: String = match addresses.last() {
        Some(last) => last.to_string(),
        None => String::from(""),
    };

    // return last address to indicate the next start_after_address
    Ok(Response::new().add_attribute("last_address", last_address))
}

/// Claim rewards as specified by the claim type, where the claim type can be
/// either Prism (claim directly as prism), Xprism (claim as xprism), or
/// Amps (claim as xprism and then bond that xprism with boost contract)
/// Any user can execute
pub fn claim_withdrawn_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    claim_type: ClaimType,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_vest(deps.storage, env.block.time.seconds(), info.sender.as_str())?;
    let amount = PENDING_WITHDRAW.load(deps.storage, info.sender.to_string().as_bytes())?;
    if amount.is_zero() {
        return Err(ContractError::InvalidClaimWithdrawnRewards {
            reason: "There are no claimable rewards".to_string(),
        });
    }

    let prism_asset = Asset {
        info: AssetInfo::Cw20(cfg.prism_token.clone()),
        amount,
    };

    let msgs = match claim_type {
        ClaimType::Prism => {
            // send prism rewards directly to user
            vec![prism_asset.transfer_msg(info.sender.clone())?]
        }
        ClaimType::Xprism => {
            // mint xprism from user's prism rewards and send xprism to the user
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.prism_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: cfg.gov.to_string(),
                    amount: prism_asset.amount,
                    msg: to_binary(&GovCw20HookMsg::MintXprism {
                        receiver: Some(info.sender.to_string()),
                    })?,
                })?,
                funds: vec![],
            })]
        }
        ClaimType::Amps => {
            // mint xprism from user's prism rewards and send xprism back to
            // this contract, then issue a BondWithBoostContractHook which will
            // bond the xprism balance difference with the xprism_boost contract.

            // we should not have any xprism balance at this point, but safer
            // to send this to the hook anyway.
            let xprism_balance =
                query_token_balance(&deps.querier, &cfg.xprism_token, &env.contract.address)?;

            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: cfg.prism_token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: cfg.gov.to_string(),
                        amount: prism_asset.amount,
                        msg: to_binary(&GovCw20HookMsg::MintXprism {
                            receiver: Some(env.contract.address.to_string()),
                        })?,
                    })?,
                    funds: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.to_string(),
                    msg: to_binary(&ExecuteMsg::BondWithBoostContractHook {
                        receiver: info.sender.clone(),
                        prev_xprism_balance: xprism_balance,
                    })?,
                    funds: vec![],
                }),
            ]
        }
    };

    // reset pending withraw to zero
    PENDING_WITHDRAW.save(
        deps.storage,
        info.sender.to_string().as_bytes(),
        &Uint128::zero(),
    )?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "claim_withdrawn_rewards")
        .add_attribute("claim_type", claim_type.to_string())
        .add_attribute("prism_reward_claimed", amount))
}

/// Hook to bond xprism with the boost contract.  This hook is invoked
/// when a user calls ClaimWithdrawnRewards with ClaimType=Amps.  For the
/// amount to bond, we use our xprism balance minus any previous balance
/// computed from the ClaimWithdrawnRewards method.  We bond with the
/// boost contract on behalf of the original claimer.  
/// Only contract can execute
pub fn bond_with_boost_contract_hook(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    receiver: Addr,
    prev_xprism_balance: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // there's no reason for anyone else to call this
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    // query our xprism balance and subtract previous balance
    let xprism_reward =
        query_token_balance(&deps.querier, &cfg.xprism_token, &env.contract.address)?
            .checked_sub(prev_xprism_balance)
            .map_err(|e| StdError::Overflow { source: e })?;

    // don't send any messages if xprism balance is zero, but not throwing
    // an error here since I guess it's plausible that a user has a single
    // prism as a reward and the MintXPrism doesn't yield any xprism
    let messages = if xprism_reward != Uint128::zero() {
        // send prism balance to boost contract and issue a bond call with
        // user set to the receiver as configured inside claim_withdraw_rewards
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.xprism_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: cfg.boost_contract.to_string(),
                amount: xprism_reward,
                msg: to_binary(&BoostContractCw20HookMsg::Bond {
                    user: Some(receiver.to_string()),
                })?,
            })?,
            funds: vec![],
        })]
    } else {
        vec![]
    };

    let res = Response::new()
        .add_messages(messages)
        .add_attribute("action", "bond_with_boost_contract_hook")
        .add_attribute("bond_amount", xprism_reward);
    Ok(res)
}
