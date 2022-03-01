use crate::contract::{_pull_pending_rewards, update_reward_indexes};
use crate::error::ContractError;
use crate::state::{CONFIG, PENDING_WITHDRAW, REWARD_INFO, SCHEDULED_VEST};
use cosmwasm_std::Addr;
use cosmwasm_std::{DepsMut, Env, MessageInfo, Order, Response, StdResult, Storage, Uint128};
use cw_asset::{Asset, AssetInfo};
use cw_storage_plus::Bound;
use prism_protocol::internal::de::deserialize_key;
use std::convert::TryInto;

// seconds in a day, make time discrete per day
pub const TIME_UNIT: u64 = 60 * 60 * 24;

// we set cap the iterations to check the vests
// in normal conditons, with a dality bulk execution,
// for most users there should be a maximum of 30 entries
pub const MAX_UPDATE_VEST_PER_TX: u64 = 50u64;

/// update_vest aggregates vested entries from SCHEDULED_VEST, removes them from
/// SCHEDULED_VEST and stores the total in PENDING_WITHDRAW.
pub fn update_vest(
    storage: &mut dyn Storage,
    current_time_seconds: u64,
    address: &str,
) -> StdResult<()> {
    let mut can_withdraw = PENDING_WITHDRAW
        .load(storage, address.as_bytes())
        .unwrap_or_else(|_| Uint128::zero());
    let mut to_delete = vec![];

    for item in SCHEDULED_VEST
        .prefix(address.as_bytes())
        .range(storage, None, None, Order::Ascending)
        .take(MAX_UPDATE_VEST_PER_TX as usize)
    {
        let (key, amount_unlocked) = item?;
        let end_time = u64::from_be_bytes(key.try_into().unwrap());
        if current_time_seconds < end_time {
            break;
        }
        can_withdraw += amount_unlocked;
        to_delete.push(end_time);
    }

    for t in to_delete {
        SCHEDULED_VEST.remove(storage, (address.as_bytes(), &t.to_be_bytes()))
    }
    PENDING_WITHDRAW.save(storage, address.as_bytes(), &can_withdraw)
}

/// withdraw_rewards starts the vesting period (rewards are not actually
/// transfered to user yet, only added to SCHEDULE_VEST to be transfered in the
/// future).
pub fn withdraw_rewards(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_reward_indexes(deps.storage, &env, &cfg)?;
    _withdraw_rewards_single(&mut deps, &env, &info.sender)
}

pub fn _withdraw_rewards_single(
    deps: &mut DepsMut,
    env: &Env,
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
        let cfg = CONFIG.load(deps.storage)?;
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
    limit: usize,
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
        .take(limit)
        .map(|k| deserialize_key::<Addr>(k).unwrap())
        .collect();

    for address in &addresses {
        _withdraw_rewards_single(&mut deps, &env, address)?;
    }

    // Return last address that was processed, for next call.
    let last_address: String = match addresses.last() {
        Some(last) => last.to_string(),
        None => String::from(""),
    };

    // return last address to indicate the next start_after_address
    Ok(Response::new().add_attribute("last_address", last_address))
}

pub fn claim_withdrawn_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_vest(deps.storage, env.block.time.seconds(), info.sender.as_str())?;
    let amount = PENDING_WITHDRAW.load(deps.storage, info.sender.to_string().as_bytes())?;
    if amount.is_zero() {
        return Err(ContractError::InvalidClaimWithdrawnRewards {
            reason: "There are no claimable rewards".to_string(),
        });
    }

    PENDING_WITHDRAW.save(
        deps.storage,
        info.sender.to_string().as_bytes(),
        &Uint128::zero(),
    )?;
    let to_withdraw = Asset {
        info: AssetInfo::Cw20(cfg.prism_token),
        amount,
    };
    Ok(Response::new().add_message(to_withdraw.transfer_msg(info.sender)?))
}
