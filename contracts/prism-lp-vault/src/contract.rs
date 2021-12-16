#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use prism_protocol::lp_vault::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode, 
};

use crate::state::{Config, RewardInfo, CONFIG, REWARD_INFO, LAST_COLLECTED};
use crate::query::{query_config, query_reward_info};
use crate::execute::{update_config, bond, unbond, claim_rewards, calculate_fees, update_rewards};

use astroport::asset::AssetInfo;
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // CONFIG.save(
    //     deps.storage,
    //     &Config {
    //         vault: deps.api.addr_validate(&msg.vault)?,
    //         gov: deps.api.addr_validate(&msg.gov)?,
    //         collector: deps.api.addr_validate(&msg.collector)?,
    //         reward_denom: msg.reward_denom,
    //         protocol_fee: msg.protocol_fee,
    //         cluna_token: deps.api.addr_validate(&msg.cluna_token)?,
    //         yluna_token: deps.api.addr_validate(&msg.yluna_token)?,
    //         pluna_token: deps.api.addr_validate(&msg.pluna_token)?,
    //         prism_token: deps.api.addr_validate(&msg.prism_token)?,
    //         withdraw_fee: msg.withdraw_fee,
    //     },
    // )?;

    Ok(Response::default())
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
        ExecuteMsg::Bond { mode } => bond(deps, env, info, mode),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, env, info),
        ExecuteMsg::CalculateFees {} => calculate_fees(deps, env, info), // should be contract restricted
        ExecuteMsg::UpdateRewards {} => update_rewards(deps, env, info), // should be contract restricted
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
