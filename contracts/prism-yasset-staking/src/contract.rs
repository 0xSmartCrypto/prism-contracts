#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};

use astroport::asset::AssetInfo;
use astroport::querier::query_token_balance;
use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    StateResponse,
};

use crate::error::{ContractError, ContractResult};
use crate::rewards::{claim_rewards, deposit_rewards, query_reward_info};
use crate::staking::{bond, unbond};
use crate::state::{Config, CONFIG, POOL_INFO};

use cw20::Cw20ReceiveMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(
        deps.storage,
        &Config {
            owner: info.sender,
            yasset_token: deps.api.addr_validate(&msg.yasset_token)?,
            reward_distribution_contract: None,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, info, amount),
        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, info),
        ExecuteMsg::DepositRewards { assets } => deposit_rewards(deps, env, info, assets),
        ExecuteMsg::PostInitialize {
            reward_distribution_contract,
        } => post_initialize(deps, env, info, reward_distribution_contract),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> ContractResult<Response> {
    let msg = cw20_msg.msg;

    match from_binary(&msg)? {
        Cw20HookMsg::Bond {} => {
            let cfg = CONFIG.load(deps.storage)?;

            // only yasset token contract can execute this message
            if cfg.yasset_token != info.sender.to_string() {
                return Err(ContractError::Unauthorized {});
            }

            bond(deps, cw20_msg.sender, cw20_msg.amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo { asset_info } => to_binary(&query_pool_info(deps, asset_info)?),
        QueryMsg::RewardInfo { staker_addr } => to_binary(&query_reward_info(deps, staker_addr)?),
        QueryMsg::State {} => to_binary(&query_state(deps, env)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        yasset_token: cfg.yasset_token.to_string(),
        reward_distribution_contract: cfg.reward_distribution_contract.map(|x| x.to_string()),
    })
}

pub fn query_pool_info(deps: Deps, asset_info: AssetInfo) -> StdResult<PoolInfoResponse> {
    let pool_info = POOL_INFO.load(deps.storage, asset_info.to_string().as_bytes())?;

    Ok(PoolInfoResponse {
        asset_info,
        reward_index: pool_info.reward_index,
    })
}

pub fn query_state(deps: Deps, env: Env) -> StdResult<StateResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    _query_state(deps, &env, &cfg)
}

pub fn _query_state(deps: Deps, env: &Env, cfg: &Config) -> StdResult<StateResponse> {
    let yasset_balance = query_token_balance(
        &deps.querier,
        cfg.yasset_token.clone(),
        env.contract.address.clone(),
    )?;

    let res = StateResponse {
        total_bond_amount: yasset_balance,
    };
    Ok(res)
}

pub fn post_initialize(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    reward_distribution_contract: String,
) -> ContractResult<Response> {
    let mut cfg = CONFIG.load(deps.storage)?;

    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    if cfg.reward_distribution_contract.is_some() {
        return Err(ContractError::DuplicatePostInitialize {});
    }
    cfg.reward_distribution_contract = Some(deps.api.addr_validate(&reward_distribution_contract)?);
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::default())
}
