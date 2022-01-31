use cosmwasm_std::{Addr, Decimal, Order, StdResult, Storage, Uint128};
use cw_storage_plus::{Bound, Item, Map, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handle::{compute_pool_reward, compute_staker_reward};

use prism_protocol::{
    de::deserialize_key,
    lp_staking::{
        ConfigResponse, PoolInfoResponse, RewardInfoResponseItem, StakerInfoResponse,
        StakersInfoResponse,
    },
};

pub const CONFIG: Item<Config> = Item::new("config");

/// staking token -> PoolInfo
pub const POOLS: Map<&Addr, PoolInfo> = Map::new("pools");

/// (staker addr, staking token) -> RewardInfo
pub const REWARD_INFO: Map<(&Addr, &Addr), RewardInfo> = Map::new("reward_info");

/// (staker addr, staking token, lock time) -> amount bonded
pub const LOCK_INFO: Map<(&Addr, &Addr, U64Key), Uint128> = Map::new("lock_info");

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
    pub lock_period: u64,
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
            lock_period: self.lock_period,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct RewardInfo {
    pub reward_index: Decimal,
    pub bond_amount: Uint128,
    pub unlocked_amount: Uint128,
    pub pending_reward: Uint128,
}

impl RewardInfo {
    pub fn as_res(&self, staking_token: &Addr) -> RewardInfoResponseItem {
        RewardInfoResponseItem {
            staking_token: staking_token.to_string(),
            bond_amount: self.bond_amount,
            pending_reward: self.pending_reward,
            withdrawable_amount: self.unlocked_amount,
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

            let (unlocked_amount, _) = get_unlocked_amount(
                storage,
                &staker_addr,
                &staking_token,
                current_time,
                pool.lock_period,
            )?;
            reward_info.unlocked_amount += unlocked_amount;

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

            let (unlocked_amount, _) = get_unlocked_amount(
                storage,
                staker,
                &staking_token,
                current_time,
                pool.lock_period,
            )?;
            reward_info.unlocked_amount += unlocked_amount;

            Ok(reward_info.as_res(&staking_token))
        })
        .collect::<StdResult<Vec<RewardInfoResponseItem>>>()
}

const DEFAULT_LOCK_INFO_READ_LIMIT: usize = 30;

pub fn get_unlocked_amount(
    storage: &dyn Storage,
    staker: &Addr,
    staking_token: &Addr,
    current_time: u64,
    lock_period: u64,
) -> StdResult<(Uint128, Vec<u64>)> {
    let (withdrawable_amount, released_locks) = LOCK_INFO
        .prefix((staker, staking_token))
        .range(storage, None, None, Order::Ascending)
        .take(DEFAULT_LOCK_INFO_READ_LIMIT)
        .fold((Uint128::zero(), vec![]), |acc, item| {
            let (k, v) = item.unwrap();
            let lock_time = deserialize_key::<u64>(k).unwrap();
            let (mut amount, mut list) = acc;

            if lock_time + lock_period < current_time {
                list.push(lock_time);
                amount += v
            }

            (amount, list)
        });

    Ok((withdrawable_amount, released_locks))
}

pub fn remove_unlocked(
    storage: &mut dyn Storage,
    staker: &Addr,
    staking_token: &Addr,
    released_locks: Vec<u64>,
) {
    for time in released_locks {
        LOCK_INFO.remove(storage, (staker, staking_token, U64Key::from(time)));
    }
}
