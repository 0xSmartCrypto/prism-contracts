#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, attr, Addr,
};

use prism_protocol::lp_vault::{
    Cw20HookMsg, Config, RewardInfo, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode, 
};

use crate::error::ContractError;
use crate::state::{CONFIG, NUM_LPS};
use crate::query::{query_config,};
use crate::execute::{update_config, bond, unbond, split, merge, stake, unstake, claim_rewards, update_staking_mode, mint, burn, create_tokens, update_rewards, post_initialize};

use astroport::asset::AssetInfo;
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let sender = info.sender.clone();
    let data = Config {
        owner: sender.to_string(),
        generator: msg.generator,
        gov: msg.gov,
        collector: msg.collector,
    };
    CONFIG.save(deps.storage, &data)?;
    NUM_LPS.save(deps.storage, &0)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        // cw20 functions
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),

        // owner functions
        ExecuteMsg::UpdateConfig { owner, generator, gov, collector, } => update_config(deps, env, info, owner, generator, gov, collector), // should be contract restricted

        // user functions
        ExecuteMsg::Merge { token, amount } => merge(deps, env, info, token, amount),
        ExecuteMsg::Split { token, amount } => split(deps, env, info, token, amount),
        ExecuteMsg::Stake { amount } => stake(deps, env, info, amount),
        ExecuteMsg::Unstake { amount } => unstake(deps, env, info, amount),
        ExecuteMsg::UpdateStakingMode { token, mode } => update_staking_mode(deps, env, info, token, mode),

        // internal functions
        ExecuteMsg::Mint { user, token, amount } => mint(deps, env, info, user, token, amount),
        ExecuteMsg::Burn { user, token, amount } => burn(deps, env, info, user, token, amount),
        ExecuteMsg::CreateTokens { } => create_tokens(deps, env, info),
        ExecuteMsg::UpdateRewards { } => update_rewards(deps, env, info), // should be contract restricted
        ExecuteMsg::PostInitialize { } => post_initialize(deps, env, info),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let cw20_sender: Addr = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => bond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
        Ok(Cw20HookMsg::Unbond {}) => unbond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
        Err(_) => Err(StdError::generic_err(format!("Invalid CW20 Message"))),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}
