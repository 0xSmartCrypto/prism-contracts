use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    Addr, CanonicalAddr, Decimal, Deps, DepsMut, Order, StdError, StdResult, Storage, Uint128,
};
use cw_storage_plus::{Bound, Item, Map, U64Key};

use prism_protocol::de::deserialize_key;
use prism_protocol::vault::{Config, State, UnbondHistory, UnbondRequest};

pub type LastBatch = u64;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Parameters {
    pub epoch_period: u64,
    pub underlying_coin_denom: String,
    pub unbonding_period: u64,
    pub peg_recovery_fee: Decimal,
    pub er_threshold: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CurrentBatch {
    pub id: u64,
    pub requested_with_fee: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const PARAMETERS: Item<Parameters> = Item::new("parameters");
pub const CURRENT_BATCH: Item<CurrentBatch> = Item::new("current_batch");
pub const STATE: Item<State> = Item::new("state");
pub const UNBOND_WAITLIST: Map<(&[u8], U64Key), Uint128> = Map::new("unbond_waitlist");
pub const UNBOND_HISTORY: Map<U64Key, UnbondHistory> = Map::new("unbond_history");
pub const VALIDATORS: Map<&[u8], bool> = Map::new("validators");

/// Store undelegation wait list per each batch
/// HashMap<user's address, <batch_id, requested_amount>
pub fn store_unbond_wait_list(
    deps: &mut DepsMut,
    batch_id: u64,
    sender_addr: String,
    amount: Uint128,
) -> StdResult<()> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr)?;
    UNBOND_WAITLIST.update(
        deps.storage,
        (sender_addr_raw.as_slice(), batch_id.into()),
        |existing_amount: Option<Uint128>| -> StdResult<_> {
            Ok(existing_amount.unwrap_or_default() + amount)
        },
    )?;
    Ok(())
}

/// Remove unbond batch id from user's wait list
pub fn remove_unbond_wait_list(
    deps: &mut DepsMut,
    batch_id: Vec<u64>,
    sender_addr: Addr,
) -> StdResult<()> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr.as_str())?;
    for b in batch_id {
        UNBOND_WAITLIST.remove(deps.storage, (sender_addr_raw.as_slice(), b.into()));
    }
    Ok(())
}

pub fn read_unbond_wait_list(
    deps: &Deps,
    batch_id: u64,
    sender_addr: String,
) -> StdResult<Uint128> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr)?;
    UNBOND_WAITLIST.load(deps.storage, (sender_addr_raw.as_slice(), batch_id.into()))
}

pub fn get_unbond_requests(deps: &Deps, sender_addr: String) -> StdResult<UnbondRequest> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr)?;
    let sender_requests: Vec<_> = UNBOND_WAITLIST
        .prefix(sender_addr_raw.as_slice())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            (batch_id, v)
        })
        .collect();
    Ok(sender_requests)
}

pub fn get_unbond_batches(deps: &Deps, sender_addr: String) -> StdResult<Vec<u64>> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr)?;
    let deprecated_batches: Vec<u64> = UNBOND_WAITLIST
        .prefix(sender_addr_raw.as_slice())
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (k, _) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            if let Ok(h) = read_unbond_history(deps.storage, batch_id) {
                if h.released {
                    Some(batch_id)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    Ok(deprecated_batches)
}

/// Return all requested unbond amount.
/// This needs to be called after process withdraw rate function.
/// If the batch is released, this will return user's requested
/// amount proportional to withdraw rate.
pub fn get_finished_amount(deps: &Deps, sender_addr: String) -> StdResult<Uint128> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr)?;
    let withdrawable_amount = UNBOND_WAITLIST
        .prefix(sender_addr_raw.as_slice())
        .range(deps.storage, None, None, Order::Ascending)
        .fold(Uint128::zero(), |acc, item| {
            let (k, v) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            if let Ok(h) = read_unbond_history(deps.storage, batch_id) {
                if h.released {
                    acc + v * h.withdraw_rate
                } else {
                    acc
                }
            } else {
                acc
            }
        });
    Ok(withdrawable_amount)
}

/// Return the finished amount for all batches that has been before the given block time.
pub fn query_get_finished_amount(
    deps: &Deps,
    sender_addr: String,
    block_time: u64,
) -> StdResult<Uint128> {
    let sender_addr_raw = deps.api.addr_canonicalize(&sender_addr)?;
    let withdrawable_amount = UNBOND_WAITLIST
        .prefix(sender_addr_raw.as_slice())
        .range(deps.storage, None, None, Order::Ascending)
        .fold(Uint128::zero(), |acc, item| {
            let (k, v) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            if let Ok(h) = read_unbond_history(deps.storage, batch_id) {
                if h.time < block_time {
                    acc + v * h.withdraw_rate
                } else {
                    acc
                }
            } else {
                acc
            }
        });
    Ok(withdrawable_amount)
}

/// Store valid validators
pub fn store_white_validators(deps: &mut DepsMut, validator_addr: String) -> StdResult<()> {
    let addr_raw = deps.api.addr_canonicalize(&validator_addr)?;
    VALIDATORS.save(deps.storage, addr_raw.as_slice(), &true)?;
    Ok(())
}

/// Remove valid validators
pub fn remove_white_validators(deps: &mut DepsMut, validator_addr: String) -> StdResult<()> {
    let addr_raw = deps.api.addr_canonicalize(&validator_addr)?;
    VALIDATORS.remove(deps.storage, addr_raw.as_slice());
    Ok(())
}

// Returns all validators
pub fn read_validators(deps: &Deps) -> StdResult<Vec<String>> {
    VALIDATORS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let canon = CanonicalAddr::from(item.unwrap().0);
            let addr = deps.api.addr_humanize(&canon)?;
            Ok(addr.into_string())
        })
        .collect()
}

/// Check whether the validator is whitelisted.
pub fn is_valid_validator(deps: &Deps, validator_addr: String) -> StdResult<bool> {
    let addr_raw = deps.api.addr_canonicalize(&validator_addr)?;
    let res = VALIDATORS.may_load(deps.storage, addr_raw.as_slice())?;
    Ok(res.is_some())
}

/// Read whitelisted validators
/// Todo: remove me, same as read_validators
pub fn read_valid_validators(deps: &Deps) -> StdResult<Vec<String>> {
    read_validators(deps)
}

pub fn store_unbond_history(
    storage: &mut dyn Storage,
    batch_id: u64,
    history: UnbondHistory,
) -> StdResult<()> {
    UNBOND_HISTORY.save(storage, batch_id.into(), &history)
}

pub fn read_unbond_history(storage: &dyn Storage, epoc_id: u64) -> StdResult<UnbondHistory> {
    UNBOND_HISTORY
        .load(storage, epoc_id.into())
        .map_err(|_| StdError::generic_err("Burn requests not found for the specified time period"))
}

// settings for pagination
const MAX_LIMIT: u32 = 100;
const DEFAULT_LIMIT: u32 = 10;

/// Return all unbond_history from UnbondHistory map
#[allow(clippy::needless_lifetimes)]
pub fn all_unbond_history(
    storage: &dyn Storage,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Vec<UnbondHistory>> {
    let start = U64Key::from(start.unwrap_or_default());
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let res = UNBOND_HISTORY
        .range(
            storage,
            Some(Bound::Exclusive(start.into())),
            None,
            Order::Ascending,
        )
        .take(limit)
        .map(|item| {
            let history: UnbondHistory = item.unwrap().1;
            Ok(history)
        })
        .collect();
    res
}
