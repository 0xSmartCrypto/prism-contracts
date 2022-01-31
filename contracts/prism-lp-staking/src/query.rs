use cosmwasm_std::{Addr, Deps, Env};

use crate::error::ContractError;
use crate::handle::{compute_pool_reward, compute_staker_reward};
use crate::state::{
    get_unlocked_amount, read_token_stakers_with_updated_rewards, read_updated_staker_rewards,
    Config, PoolInfo, RewardInfo, CONFIG, POOLS, REWARD_INFO,
};

use prism_protocol::lp_staking::{
    ConfigResponse, PoolInfoResponse, RewardInfoResponseItem, StakerInfoResponse,
    StakersInfoResponse,
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
            let (unlocked_amount, _) = get_unlocked_amount(
                deps.storage,
                &staker,
                &staking_token,
                env.block.time.seconds(),
                pool.lock_period,
            )?;
            reward_info.unlocked_amount += unlocked_amount;

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
