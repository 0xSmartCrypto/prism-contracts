use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env};

use crate::error::ContractError;
use crate::handle::{compute_pool_reward, compute_staker_reward};
use crate::state::{
    read_token_stakers_with_updated_rewards, read_updated_staker_rewards, Config, PoolInfo,
    RewardInfo, CONFIG, POOLS, REWARD_INFO,
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
    let staking_token_raw: CanonicalAddr = deps.api.addr_canonicalize(staking_token.as_str())?;
    let pool: PoolInfo = POOLS.load(deps.storage, staking_token_raw.as_slice())?;

    Ok(pool.as_res(&staking_token))
}

pub fn query_staker_info(
    deps: Deps,
    env: Env,
    staker: Addr,
    staking_token: Option<Addr>,
) -> Result<StakerInfoResponse, ContractError> {
    let staker_raw: CanonicalAddr = deps.api.addr_canonicalize(staker.as_str())?;

    let staker_rewards: Vec<RewardInfoResponseItem> = match staking_token {
        Some(staking_token) => {
            let config: Config = CONFIG.load(deps.storage)?;
            let staking_token_raw: CanonicalAddr =
                deps.api.addr_canonicalize(staking_token.as_str())?;
            let mut pool: PoolInfo = POOLS.load(deps.storage, staking_token_raw.as_slice())?;
            let mut reward_info: RewardInfo = REWARD_INFO.load(
                deps.storage,
                (staker_raw.as_slice(), staking_token_raw.as_slice()),
            )?;

            compute_pool_reward(&config, &mut pool, env.block.time.seconds());
            compute_staker_reward(&pool, &mut reward_info);

            vec![reward_info.as_res(&staking_token)]
        }
        None => read_updated_staker_rewards(
            deps.storage,
            deps.api,
            env.block.time.seconds(),
            staker_raw,
        )?,
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
    let staking_token_raw: CanonicalAddr = deps.api.addr_canonicalize(staking_token.as_str())?;
    let start_after: Option<CanonicalAddr> =
        start_after.map(|addr| deps.api.addr_canonicalize(addr.as_str()).unwrap());

    let res: StakersInfoResponse = read_token_stakers_with_updated_rewards(
        deps.storage,
        deps.api,
        env.block.time.seconds(),
        staking_token_raw,
        start_after,
        limit,
    )?;

    Ok(res)
}
