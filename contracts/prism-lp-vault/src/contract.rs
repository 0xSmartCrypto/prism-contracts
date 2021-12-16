#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, attr
};

use prism_protocol::lp_vault::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode, 
};

use crate::state::{Config, RewardInfo, CONFIG, REWARD_INFO, LAST_COLLECTED};
use crate::query::{query_config, query_reward_info};
use crate::execute::{update_config, bond, unbond, claim_rewards, update_staking_mode, calculate_fees, update_rewards};

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
        vault: msg.vault,
        gov: msg.gov,
        collector: msg.collector,
        collect_period: msg.collect_period,
    };
    CONFIG.save(deps.storage, &data)?;

    let init_collected: u64 = 0;
    LAST_COLLECTED.save(deps.storage, &init_collected)?;

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
        //ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::UpdateConfig { owner, vault, gov, collector, collect_period } => update_config(deps, env, info, owner, vault, gov, collector, collect_period), // should be contract restricted
        ExecuteMsg::Bond { token, mode } => bond(deps, env, info, token, mode),
        ExecuteMsg::Unbond { token, amount } => unbond(deps, env, info, token, amount),
        ExecuteMsg::ClaimRewards {token, } => claim_rewards(deps, env, info, token),
        ExecuteMsg::UpdateStakingMode { token, mode } => update_staking_mode(deps, env, info, token, mode),
        ExecuteMsg::CalculateFees {user, token, } => calculate_fees(deps, env, info, user, token), // should be contract restricted
        ExecuteMsg::UpdateRewards {user, token} => update_rewards(deps, env, info, user, token), // should be contract restricted
    }
}

// might need to do this for yLP tokens
// pub fn receive_cw20(
//     deps: DepsMut,
//     info: MessageInfo,
//     cw20_msg: Cw20ReceiveMsg,
// ) -> StdResult<Response<TerraMsgWrapper>> {
//     let msg = cw20_msg.msg;

//     match from_binary(&msg)? {
//         Cw20HookMsg::Bond { mode } => {
//             let cfg = CONFIG.load(deps.storage)?;

//             // only yluna token contract can execute this message
//             if cfg.yluna_token != info.sender.to_string() {
//                 return Err(StdError::generic_err("unauthorized"));
//             }

//             bond(deps, cw20_msg.sender, cw20_msg.amount, mode)
//         }
//     }
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::GetRewardInfo { stakerAddr, tokenAddr } => to_binary(&query_reward_info(deps, stakerAddr, tokenAddr)?),
    }
}
