#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult,
};

use cw20::Cw20ReceiveMsg;

use prism_protocol::lp_vault::{Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::bond::{bond, create_tokens, unbond};
use crate::query::query_config;
use crate::refract::{merge, split};
use crate::stake::{
    claim_rewards, send_staker_rewards, stake, unstake, update_lp_rewards, update_staker_info,
    update_staking_mode,
};
use crate::state::{CONFIG, NUM_LPS};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let data = Config {
        owner: info.sender.to_string(),
        generator: msg.generator,
        factory: msg.factory,
        collector: msg.collector,
        fee: msg.fee,
    };
    CONFIG.save(deps.storage, &data)?;
    NUM_LPS.save(deps.storage, &0)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        // owner functions
        ExecuteMsg::UpdateConfig {
            owner,
            generator,
            factory,
            collector,
            fee,
        } => update_config(deps, info, owner, generator, factory, collector, fee),

        // user functions
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),

        ExecuteMsg::Merge { token, amount } => merge(deps, info, token, amount),

        ExecuteMsg::Split { token, amount } => split(deps, info, token, amount),

        ExecuteMsg::Unstake { token, amount } => unstake(deps, env, info, token, amount),

        ExecuteMsg::UpdateStakingMode { token, mode } => {
            update_staking_mode(deps, info, token, mode)
        }

        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, env, info),

        // internal functions
        ExecuteMsg::CreateTokens { token } => create_tokens(deps, env, info, token),

        ExecuteMsg::UpdateLPRewards { token } => update_lp_rewards(deps, env, info, token),

        ExecuteMsg::SendStakerRewards { staker } => send_staker_rewards(deps, env, info, staker),

        ExecuteMsg::UpdateStakerInfo {
            lp_id,
            sender_addr,
            amount,
            stake,
        } => update_staker_info(deps, env, info, lp_id, sender_addr, amount, stake),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let cw20_sender: Addr = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond {} => bond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
        Cw20HookMsg::Unbond {} => unbond(deps, info.sender, cw20_sender, cw20_msg.amount),
        Cw20HookMsg::Stake {} => stake(deps, env, info.sender, cw20_sender, cw20_msg.amount),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        // provide query for individual StakerInfo
        // provide query for all StakerInfo for an individual user
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    generator: Option<String>,
    factory: Option<String>,
    collector: Option<String>,
    fee: Option<Decimal>,
) -> StdResult<Response> {
    let mut conf = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != conf.owner {
        return Err(StdError::generic_err("Unauthorized".to_string()));
    }

    conf.owner = owner.unwrap_or(conf.owner);
    conf.generator = generator.unwrap_or(conf.generator);
    conf.factory = factory.unwrap_or(conf.factory);
    conf.collector = collector.unwrap_or(conf.collector);
    conf.fee = fee.unwrap_or(conf.fee);
    CONFIG.save(deps.storage, &conf)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}
