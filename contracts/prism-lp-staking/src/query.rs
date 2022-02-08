use cosmwasm_std::{Addr, Deps, Env, Order, StdResult, Uint128};
use cw_storage_plus::{Bound, U64Key};
use prism_protocol::internal::de::deserialize_key;

use crate::error::ContractError;
use crate::handle::{compute_pool_reward, compute_staker_reward};
use crate::state::{
    get_withdrawable_amount, read_token_stakers_with_updated_rewards, read_updated_staker_rewards,
    Config, PoolInfo, RewardInfo, CONFIG, POOLS, REWARD_INFO, UNBOND_ORDERS,
};

use prism_protocol::lp_staking::{
    ConfigResponse, PoolInfoResponse, RewardInfoResponseItem, StakerInfoResponse,
    StakersInfoResponse, UnbondOrdersResponse,
};

pub fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(config.as_res()?)
}

pub fn query_pool_info(deps: Deps, staking_token: Addr) -> Result<PoolInfoResponse, ContractError> {
    let pool: PoolInfo = POOLS.load(deps.storage, &staking_token)?;

    Ok(pool.as_res(&staking_token))
}

pub fn query_staker_info(
    deps: Deps,
    env: Env,
    staker: Addr,
    staking_token: Option<Addr>,
) -> Result<StakerInfoResponse, ContractError> {
    let staker_rewards: Vec<RewardInfoResponseItem> = match staking_token {
        Some(staking_token) => {
            let config: Config = CONFIG.load(deps.storage)?;
            let mut pool: PoolInfo = POOLS.load(deps.storage, &staking_token)?;
            let mut reward_info: RewardInfo =
                REWARD_INFO.load(deps.storage, (&staker, &staking_token))?;

            // update the unlocked_amount
            let (withdrawable_amount, _, _) = get_withdrawable_amount(
                deps.storage,
                &staker,
                &staking_token,
                env.block.time.seconds(),
                pool.unbond_period,
            )?;
            reward_info.withdrawable_amount += withdrawable_amount;

            compute_pool_reward(&config, &mut pool, env.block.time.seconds());
            compute_staker_reward(&pool, &mut reward_info);

            vec![reward_info.as_res(&staking_token)]
        }
        None => read_updated_staker_rewards(deps.storage, env.block.time.seconds(), &staker)?,
    };

    Ok(StakerInfoResponse {
        staker: staker.to_string(),
        reward_infos: staker_rewards,
    })
}

pub fn query_token_stakers_info(
    deps: Deps,
    env: Env,
    staking_token: Addr,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> Result<StakersInfoResponse, ContractError> {
    let res: StakersInfoResponse = read_token_stakers_with_updated_rewards(
        deps.storage,
        env.block.time.seconds(),
        staking_token,
        start_after,
        limit,
    )?;

    Ok(res)
}

const MAX_ORDER_LIMIT: u32 = 50u32;
const DEFAULT_ORDER_LIMIT: u32 = 30u32;
pub fn query_unbond_orders(
    deps: Deps,
    env: Env,
    staking_token: Addr,
    staker: Addr,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<UnbondOrdersResponse> {
    let pool: PoolInfo = POOLS.load(deps.storage, &staking_token)?;

    let limit = limit.unwrap_or(DEFAULT_ORDER_LIMIT).min(MAX_ORDER_LIMIT) as usize;
    let start = start_after.map(|start| Bound::exclusive(U64Key::from(start)));

    let current_time = env.block.time.seconds();
    let mut withdrawable_amount = Uint128::zero();
    let orders: Vec<(u64, Uint128)> = UNBOND_ORDERS
        .prefix((&staker, &staking_token))
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (key, amount) = item?;
            let order_time = deserialize_key::<u64>(key).unwrap();
            let end_time = order_time + pool.unbond_period;
            if end_time < current_time {
                withdrawable_amount += amount;
            }

            Ok((end_time, amount))
        })
        .collect::<StdResult<Vec<(u64, Uint128)>>>()?;

    Ok(UnbondOrdersResponse {
        withdrawable_amount,
        orders,
    })
}
