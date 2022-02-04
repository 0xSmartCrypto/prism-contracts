use cosmwasm_std::{Addr, Decimal, Order, StdResult, Storage, Uint128};
use cw_storage_plus::{Bound, Item, Map, U64Key};
use prism_protocol::internal::de::deserialize_key;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handle::{compute_pool_reward, compute_staker_reward};

use prism_protocol::lp_staking::{
    ConfigResponse, PoolInfoResponse, RewardInfoResponseItem, StakerInfoResponse,
    StakersInfoResponse,
};

pub const CONFIG: Item<Config> = Item::new("config");

/// staking token -> PoolInfo
pub const POOLS: Map<&Addr, PoolInfo> = Map::new("pools");

/// (staker addr, staking token) -> RewardInfo
pub const REWARD_INFO: Map<(&Addr, &Addr), RewardInfo> = Map::new("reward_info");

/// (staker addr, staking token, order_creation_time) -> amount requtested to unbond
pub const UNBOND_ORDERS: Map<(&Addr, &Addr, U64Key), Uint128> = Map::new("unbond_orders");

/// (staking token, staker addr) -> bool
pub const STAKER_BY_TOKEN_INDEXER: Map<(&Addr, &Addr), bool> = Map::new("staker_by_token");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub prism_token: Addr,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
    pub total_weight: u64,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: self.owner.to_string(),
            prism_token: self.prism_token.to_string(),
            distribution_schedule: self.distribution_schedule.clone(),
            total_weight: self.total_weight,
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct PoolInfo {
    pub last_distributed: u64,
    pub weight: u64,
    pub total_bond_amount: Uint128,
    pub reward_index: Decimal,
    pub pending_reward: Uint128,
    pub unbond_period: u64,
}

impl PoolInfo {
    pub fn as_res(&self, staking_token: &Addr) -> PoolInfoResponse {
        PoolInfoResponse {
            staking_token: staking_token.to_string(),
            weight: self.weight,
            last_distributed: self.last_distributed,
            total_bond_amount: self.total_bond_amount,
            reward_index: self.reward_index,
            pending_reward: self.pending_reward,
            unbond_period: self.unbond_period,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub withdrawable_amount: Uint128,
    pub pending_reward: Uint128,
}

impl RewardInfo {
    pub fn as_res(&self, staking_token: &Addr) -> RewardInfoResponseItem {
        RewardInfoResponseItem {
            staking_token: staking_token.to_string(),
            bond_amount: self.bond_amount,
            pending_reward: self.pending_reward,
            withdrawable_amount: self.withdrawable_amount,
        }
    }
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn read_token_stakers_with_updated_rewards(
    storage: &dyn Storage,
    current_time: u64,
    staking_token: Addr,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<StakersInfoResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|addr| Bound::exclusive(addr.as_bytes()));

    let config: Config = CONFIG.load(storage)?;
    let mut pool: PoolInfo = POOLS.load(storage, &staking_token)?;
    compute_pool_reward(&config, &mut pool, current_time);

    let stakers: Vec<StakerInfoResponse> = STAKER_BY_TOKEN_INDEXER
        .prefix(&staking_token)
        .range(storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (k, _) = item?;
            let staker_addr = deserialize_key::<Addr>(k).unwrap();
            let mut reward_info: RewardInfo =
                REWARD_INFO.load(storage, (&staker_addr, &staking_token))?;

            let (withdrawable_amount, _, _) = get_withdrawable_amount(
                storage,
                &staker_addr,
                &staking_token,
                current_time,
                pool.unbond_period,
            )?;
            reward_info.withdrawable_amount += withdrawable_amount;

            compute_staker_reward(&pool, &mut reward_info);

            Ok(StakerInfoResponse {
                staker: staker_addr.to_string(),
                reward_infos: vec![reward_info.as_res(&staking_token)],
            })
        })
        .collect::<StdResult<Vec<StakerInfoResponse>>>()?;

    Ok(StakersInfoResponse { stakers })
}

pub fn read_updated_staker_rewards(
    storage: &dyn Storage,
    current_time: u64,
    staker: &Addr,
) -> StdResult<Vec<RewardInfoResponseItem>> {
    let config: Config = CONFIG.load(storage)?;

    REWARD_INFO
        .prefix(staker)
        .range(storage, None, None, Order::Ascending)
        .map(|item| {
            let (k, mut reward_info) = item.unwrap();
            let staking_token = deserialize_key::<Addr>(k).unwrap();
            let mut pool = POOLS.load(storage, &staking_token)?;
            compute_pool_reward(&config, &mut pool, current_time);
            compute_staker_reward(&pool, &mut reward_info);

            let (withdrawable_amount, _, _) = get_withdrawable_amount(
                storage,
                staker,
                &staking_token,
                current_time,
                pool.unbond_period,
            )?;
            reward_info.withdrawable_amount += withdrawable_amount;

            Ok(reward_info.as_res(&staking_token))
        })
        .collect::<StdResult<Vec<RewardInfoResponseItem>>>()
}

const MAX_ORDER_WITHDRAW_PER_TX: usize = 50usize;

pub fn get_withdrawable_amount(
    storage: &dyn Storage,
    staker: &Addr,
    staking_token: &Addr,
    current_time: u64,
    unbond_period: u64,
) -> StdResult<(Uint128, Vec<u64>, u64)> {
    // We use order_count and released_locks length to check if there are more withdraw orders pending.
    // To cover the edge case when we unlock MAX_ORDER_WITHDRAW_PER_TX, we do one more iteration just
    // to check if there are other orders over the limit.

    let (withdrawable_amount, released_locks, order_count) = UNBOND_ORDERS
        .prefix((staker, staking_token))
        .range(storage, None, None, Order::Ascending)
        .take(MAX_ORDER_WITHDRAW_PER_TX + 1)
        .fold((Uint128::zero(), vec![], 0u64), |acc, item| {
            let (k, v) = item.unwrap();
            let order_time = deserialize_key::<u64>(k).unwrap();
            let (mut amount, mut list, mut count) = acc;

            count += 1u64;
            if count <= MAX_ORDER_WITHDRAW_PER_TX as u64
                && order_time + unbond_period < current_time
            {
                list.push(order_time);
                amount += v
            }
            (amount, list, count)
        });

    Ok((withdrawable_amount, released_locks, order_count))
}

pub fn remove_withdrawn(
    storage: &mut dyn Storage,
    staker: &Addr,
    staking_token: &Addr,
    expired_orders: Vec<u64>,
) {
    for time in expired_orders {
        UNBOND_ORDERS.remove(storage, (staker, staking_token, U64Key::from(time)));
    }
}
