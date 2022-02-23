use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Order, StdError, StdResult, Storage, Uint128};
use cw_storage_plus::{Bound, Item, Map, U64Key};

use prism_protocol::{
    internal::de::deserialize_key,
    vault::{ConfigResponse, StateResponse, UnbondHistoryResponse},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Parameters {
    pub epoch_period: u64, // as a duration in seconds
    pub underlying_coin_denom: String,
    pub unbonding_period: u64,     // as a duration in seconds
    pub peg_recovery_fee: Decimal, // must be in [0, 1].
    pub er_threshold: Decimal,     // exchange rate threshold. Must be in [0, 1].
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CurrentBatch {
    pub id: u64,
    pub requested_with_fee: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct State {
    pub exchange_rate: Decimal,
    pub total_bond_amount: Uint128,
    pub last_index_modification: u64,
    pub prev_vault_balance: Uint128,
    pub actual_unbonded_amount: Uint128,
    pub last_unbonded_time: u64,
    pub last_processed_batch: u64,
}

impl State {
    pub fn update_exchange_rate(&mut self, total_issued: Uint128, requested_with_fee: Uint128) {
        let actual_supply = total_issued + requested_with_fee;
        if self.total_bond_amount.is_zero() || actual_supply.is_zero() {
            self.exchange_rate = Decimal::one()
        } else {
            self.exchange_rate = Decimal::from_ratio(self.total_bond_amount, actual_supply);
        }
    }

    pub fn as_res(&self) -> StateResponse {
        StateResponse {
            exchange_rate: self.exchange_rate,
            total_bond_amount: self.total_bond_amount,
            last_index_modification: self.last_index_modification,
            prev_vault_balance: self.prev_vault_balance,
            actual_unbonded_amount: self.actual_unbonded_amount,
            last_unbonded_time: self.last_unbonded_time,
            last_processed_batch: self.last_processed_batch,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // owner is the address of the owner of the Vault. It is used to
    // authenticate owner-only endpoints (an error will be returned if this
    // field doesn't match the caller's address).
    pub owner: Addr,
    // yluna_staking is the address of the yasset-staking contract. If set,
    // delegation rewards are deposited directly there (via SetWithdrawAddress).
    // Example: Alice calls Bond on the Vault with 1 Luna. The Vault delegates
    // that Luna to a validator. Rewards from that delegation go straight to the
    // yasset-staking contract, bypassing the Vault completely.
    pub yluna_staking: Addr,
    // cluna_contract, yluna_contract and pluna_contract are the addresses of
    // the corresponding CW20 contracts. They are needed to mint, burn and
    // transfer these tokens.
    pub cluna_contract: Addr,
    pub yluna_contract: Addr,
    pub pluna_contract: Addr,
    pub airdrop_registry_contract: Addr,
    // initialized indicates whether the Vault is fully initialized and ready to
    // be used. It is needed to break a cyclical dependency during contract
    // initialization (Vault needs yasset-staking's address, but yasset-staking
    // needs Vault's address, so we break the cycle by instantiating Vault first
    // with initialized=false).
    pub initialized: bool,
    pub token_admin: Addr,
    pub token_code_id: u64,
    pub manager: Addr,
}

impl Config {
    pub fn as_res(&self) -> ConfigResponse {
        ConfigResponse {
            owner: self.owner.to_string(),
            yluna_staking: self.yluna_staking.to_string(),
            cluna_contract: self.cluna_contract.to_string(),
            yluna_contract: self.yluna_contract.to_string(),
            pluna_contract: self.pluna_contract.to_string(),
            airdrop_registry_contract: self.airdrop_registry_contract.to_string(),
            initialized: self.initialized,
            manager: self.manager.to_string(),
        }
    }

    pub fn assert_initialized(self) -> StdResult<Config> {
        if self.initialized {
            Ok(self)
        } else {
            Err(StdError::generic_err(
                "Contract initialization is not completed",
            ))
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondHistory {
    pub batch_id: u64,
    pub time: u64,
    pub amount: Uint128,
    pub applied_exchange_rate: Decimal,
    pub withdraw_rate: Decimal,
    pub released: bool,
}

impl UnbondHistory {
    pub fn as_res(&self) -> UnbondHistoryResponse {
        UnbondHistoryResponse {
            batch_id: self.batch_id,
            time: self.time,
            amount: self.amount,
            applied_exchange_rate: self.applied_exchange_rate,
            withdraw_rate: self.withdraw_rate,
            released: self.released,
        }
    }
}

pub type UnbondRequest = Vec<(u64, Uint128)>;

pub const CONFIG: Item<Config> = Item::new("config");
pub const PARAMETERS: Item<Parameters> = Item::new("parameters");
pub const CURRENT_BATCH: Item<CurrentBatch> = Item::new("current_batch");
pub const STATE: Item<State> = Item::new("state");
pub const UNBOND_WAITLIST: Map<(&Addr, U64Key), Uint128> = Map::new("unbond_waitlist");
pub const UNBOND_HISTORY: Map<U64Key, UnbondHistory> = Map::new("unbond_history");
pub const VALIDATORS: Map<&Addr, bool> = Map::new("validators");

/// Store undelegation wait list per each batch
/// HashMap<user's address + batch_id, requested_amount>
pub fn store_unbond_wait_list(
    storage: &mut dyn Storage,
    batch_id: u64,
    sender_addr: &Addr,
    amount: Uint128,
) -> StdResult<()> {
    UNBOND_WAITLIST.update(
        storage,
        (sender_addr, batch_id.into()),
        |existing_amount: Option<Uint128>| -> StdResult<_> {
            Ok(existing_amount.unwrap_or_default() + amount)
        },
    )?;
    Ok(())
}

/// Remove unbond batch id from user's wait list
pub fn remove_unbond_wait_list(
    storage: &mut dyn Storage,
    batch_id: Vec<u64>,
    sender_addr: &Addr,
) -> StdResult<()> {
    for b in batch_id {
        UNBOND_WAITLIST.remove(storage, (sender_addr, b.into()));
    }
    Ok(())
}

pub fn read_unbond_wait_list(
    storage: &dyn Storage,
    batch_id: u64,
    sender_addr: &Addr,
) -> StdResult<Uint128> {
    UNBOND_WAITLIST.load(storage, (sender_addr, batch_id.into()))
}

const DEFAULT_UNBOND_WAITLIST_READ_LIMIT: u32 = 30u32;

pub fn get_unbond_requests(
    storage: &dyn Storage,
    sender_addr: &Addr,
    start: Option<u64>,
    limit: Option<u32>,
) -> StdResult<UnbondRequest> {
    let start = U64Key::from(start.unwrap_or_default());

    let sender_requests: Vec<_> = UNBOND_WAITLIST
        .prefix(sender_addr)
        .range(
            storage,
            Some(Bound::Exclusive(start.into())),
            None,
            Order::Ascending,
        )
        .take(
            limit
                .unwrap_or(DEFAULT_UNBOND_WAITLIST_READ_LIMIT)
                .min(MAX_LIMIT) as usize,
        )
        .map(|item| {
            let (k, v) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            (batch_id, v)
        })
        .collect();
    Ok(sender_requests)
}

pub fn get_unbond_batches(
    storage: &dyn Storage,
    sender_addr: &Addr,
    limit: Option<u32>,
) -> StdResult<Vec<u64>> {
    let deprecated_batches: Vec<u64> = UNBOND_WAITLIST
        .prefix(sender_addr)
        .range(storage, None, None, Order::Ascending)
        .take(
            limit
                .unwrap_or(DEFAULT_UNBOND_WAITLIST_READ_LIMIT)
                .min(MAX_LIMIT) as usize,
        )
        .filter_map(|item| {
            let (k, _) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            if let Ok(h) = read_unbond_history(storage, batch_id) {
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
pub fn get_finished_amount(
    storage: &dyn Storage,
    sender_addr: &Addr,
    limit: Option<u32>,
) -> StdResult<Uint128> {
    let withdrawable_amount = UNBOND_WAITLIST
        .prefix(sender_addr)
        .range(storage, None, None, Order::Ascending)
        .take(
            limit
                .unwrap_or(DEFAULT_UNBOND_WAITLIST_READ_LIMIT)
                .min(MAX_LIMIT) as usize,
        )
        .fold(Uint128::zero(), |acc, item| {
            let (k, v) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            if let Ok(h) = read_unbond_history(storage, batch_id) {
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
    storage: &dyn Storage,
    sender_addr: &Addr,
    block_time: u64,
    limit: Option<u32>,
) -> StdResult<Uint128> {
    let withdrawable_amount = UNBOND_WAITLIST
        .prefix(sender_addr)
        .range(storage, None, None, Order::Ascending)
        .take(
            limit
                .unwrap_or(DEFAULT_UNBOND_WAITLIST_READ_LIMIT)
                .min(MAX_LIMIT) as usize,
        )
        .fold(Uint128::zero(), |acc, item| {
            let (k, v) = item.unwrap();
            let batch_id = deserialize_key::<u64>(k).unwrap();
            if let Ok(h) = read_unbond_history(storage, batch_id) {
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
pub fn store_white_validators(storage: &mut dyn Storage, validator_addr: &Addr) -> StdResult<()> {
    VALIDATORS.save(storage, validator_addr, &true)?;
    Ok(())
}

/// Remove valid validators
pub fn remove_white_validators(storage: &mut dyn Storage, validator_addr: &Addr) -> StdResult<()> {
    VALIDATORS.remove(storage, validator_addr);
    Ok(())
}

// Returns all validators
pub fn read_validators(storage: &dyn Storage) -> StdResult<Vec<Addr>> {
    VALIDATORS
        .range(storage, None, None, Order::Ascending)
        .map(|item| deserialize_key::<Addr>(item.unwrap().0))
        .collect()
}

/// Check whether the validator is whitelisted.
pub fn is_valid_validator(storage: &dyn Storage, validator_addr: &Addr) -> StdResult<bool> {
    let res = VALIDATORS.may_load(storage, validator_addr)?;
    Ok(res.is_some())
}

/// Read whitelisted validators
/// Todo: remove me, same as read_validators
pub fn read_valid_validators(storage: &dyn Storage) -> StdResult<Vec<Addr>> {
    read_validators(storage)
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
