#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg, Reply, ReplyOn, Decimal,
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::generator::{Cw20HookMsg as AstroHookMsg, ExecuteMsg as AstroExecuteMsg};
use astroport::token::{InstantiateMsg as AstroTokenInstantiateMsg};
use astroport::factory::{ConfigResponse as FactoryConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS, LPInfo};
use crate::query::{query_config, query_token_info, query_pair_info, query_factory_config};

use crate::response::MsgInstantiateContractResponse;
use protobuf::Message;

use astroport::asset::{AssetInfo, addr_validate_to_lower};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, TokenInfoResponse, MinterResponse};
use terra_cosmwasm::TerraMsgWrapper;



// pub struct StakerInfo {
//     pub amt_bonded: Uint128,
//     pub mode: StakingMode,
//     pub reward_info: RewardInfo,
// }

// change to vec of 2 assetinfos to match astro?
// pub struct RewardInfo {
//     pub pending_underlying_reward_1: Uint128,
//     pub pending_underlying_reward_2: Uint128,
//     pub pending_underlying_astro: Uint128,
// }

// TODO
// only callable by cw20
pub fn stake(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    // check that addr exists internally
    // check that the addr sent is a yLP token (and not p/cLP) via LPInfo

    // call update rewards
    // call update staker

    // check for (lp_id, user) staker_info
    // if exists, add bond amount
    // else, create new StakerInfo with bond amount and store
    Ok(Response::new())
}

// TODO
pub fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    // check that addr exists internally
    // check that token sent is a yLP token via LPInfo
    
    // call update rewards
    // call update staker

    // check for (lp_id, user) staker_info
    // if doesn't exist or amount < whats available, error
    // if amount is empty, do all bonded yLP
    // if bond amount is empty and RewardInfo is empty, delete StakerInfo instance
    Ok(Response::new())
}

// TODO
pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    // call update_rewards
    // call send_rewards

    // for each {info.sender, token_id} in STAKER_INFO

    // send back all rewards (make a helper per RewardInfo)

    // delete StakerInfo instance iff amt_bonded is empty

    Ok(Response::new())
}

// TODO
pub fn update_staking_mode(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    mode: StakingMode,
) -> StdResult<Response> {
    // call update_rewards

    // send tokens

    // check that {user, token} StakerInfo exists

    // update StakingMode
    Ok(Response::new())
}

// TODO
pub fn update_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    // update RewardInfo of given LP token

    // grab x and y from Astroport, amt_bonded and last liquidity from LP_INFO
    // calculate new liquidity, calculate amount of LP to withdraw and burn

    // might want to query reward too

    // for each {user, token_id} in STAKER_INFO
    // if default mode, add underlying rewards
    // if xprism mode, use collector contract to convert and add to xprism reward

    Ok(Response::new())
}