#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg, Reply, ReplyOn, Decimal, Order
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode,
};

use astroport::asset::{AssetInfo, Asset, addr_validate_to_lower};
use astroport::generator::{ExecuteMsg as AstroGenExecuteMsg, PendingTokenResponse};
use astroport::pair::{Cw20HookMsg as AstroPairHookMsg};
use astroport::token::{InstantiateMsg as AstroTokenInstantiateMsg};
use astroport::factory::{ConfigResponse as FactoryConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS, LPInfo, STAKER_INFO};
use crate::query::{query_config, query_token_info, query_pair_info, query_factory_config, query_pending_generator_rewards, query_pool_info, query_lp_burn_rewards};
use crate::math::decimal_division;

use crate::response::MsgInstantiateContractResponse;
use protobuf::Message;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, TokenInfoResponse, MinterResponse};
use terra_cosmwasm::TerraMsgWrapper;

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

pub fn update_lp_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    let lp_id = LP_IDS.load(deps.storage, &token.clone())?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;
    let vault_share = lp_info.amt_bonded.clone();

    // calculate and withdraw astro generator rewards
    let mut pending_gen_rewards: PendingTokenResponse = query_pending_generator_rewards(deps.as_ref(), env, &deps.querier, token.clone())?;
    let mut pending_proxy = Uint128::zero();
    if pending_gen_rewards.pending_on_proxy != None {
        pending_proxy = pending_gen_rewards.pending_on_proxy.unwrap();
    }

    let mut messages = vec![];
    if pending_gen_rewards.pending > Uint128::zero() || pending_proxy > Uint128::zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.generator.clone(),
            msg: to_binary(&AstroGenExecuteMsg::Withdraw {
                lp_token: token.clone(),
                amount: Uint128::zero(),
            })?,
            funds: vec![],
        }));
    }

    // calculate and withdraw AMM rewards
    let pool_info = query_pool_info(deps.as_ref(), &deps.querier, token.clone())?;

    // why is math so hard to do here
    // s = liquidity per token = sqrt(xy)/number of LP
    // withdraw and burn (1 - s_last/s_new)*vault_share of LP tokens
    let s = Decimal::from_ratio(pool_info.assets[0].amount * pool_info.assets[1].amount, Uint128::new(1)).sqrt();
    let new_liquidity: Decimal = s / pool_info.total_share;
    let inv_new_liquidity = decimal_division(Uint128::new(1), new_liquidity);
    let inv_last_liquidity = decimal_division(Uint128::new(1), lp_info.last_liquidity);
    let tokens_to_burn = (Uint128::new(1).checked_sub(inv_new_liquidity / inv_last_liquidity)?) * lp_info.amt_bonded;

    let mut pending_amm_rewards: Vec<Asset> = query_lp_burn_rewards(deps.as_ref(), &deps.querier, token.clone(), tokens_to_burn.clone())?;

    if pending_amm_rewards[0].amount > Uint128::zero() || pending_amm_rewards[1].amount > Uint128::zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token.clone().into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: lp_info.pair_contract.clone().into_string(),
                msg: to_binary(&AstroPairHookMsg::WithdrawLiquidity {})?,
                amount: tokens_to_burn,
            })?,
            funds: vec![],
        }));
    }

    // exit early if no pending rewards
    if !(pending_gen_rewards.pending > Uint128::zero() ||
         pending_proxy > Uint128::zero() ||
         pending_amm_rewards[0].amount > Uint128::zero() ||
         pending_amm_rewards[1].amount > Uint128::zero()) {
             return Ok(Response::new());
    }

    // deduct fees and send to collector if applicable
    // ASTRO reward from generator
    if pending_gen_rewards.pending > Uint128::zero() {
        let prism_fee = pending_gen_rewards.pending * config.fee;
        let reward_asset = Asset {
            info: lp_info.generator_reward_info[0].clone(),
            amount: prism_fee
        };
        pending_gen_rewards.pending = pending_gen_rewards.pending.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, Addr::unchecked(config.collector.clone()))?);
    }

    // proxy reward from generator
    if pending_proxy > Uint128::zero() {
        let prism_fee = pending_proxy * config.fee;
        let reward_asset = Asset {
            info: lp_info.generator_reward_info[1].clone(),
            amount: prism_fee
        };
        pending_proxy = pending_proxy.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, Addr::unchecked(config.collector.clone()))?);
    }

    // first underlying AMM reward
    if pending_amm_rewards[0].amount > Uint128::zero() {
        let prism_fee = pending_amm_rewards[0].clone().amount * config.fee;
        let reward_asset = Asset {
            info: pending_amm_rewards[0].clone().info,
            amount: prism_fee,
        };
        pending_amm_rewards[0].amount = pending_amm_rewards[0].amount.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, Addr::unchecked(config.collector.clone()))?);
    }

    // second underlying AMM reward
    if pending_amm_rewards[1].amount > Uint128::zero() {
        let prism_fee = pending_amm_rewards[1].clone().amount * config.fee;
        let reward_asset = Asset {
            info: pending_amm_rewards[1].clone().info,
            amount: prism_fee,
        };
        pending_amm_rewards[1].amount = pending_amm_rewards[1].amount.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, Addr::unchecked(config.collector.clone()))?);
    }

    // get all stakers infos for this LP
    let all_stakers: Vec<Addr> = STAKER_INFO
               .prefix(lp_id.into())
               .range(deps.storage, None, None, Order::Ascending)
               .map(|item| {
                    let (staker, _) = item.unwrap();
                    deps.api.addr_validate(&String::from_utf8(staker).unwrap()).unwrap()
               })
               .collect();
    
    // update reward infos
    for staker in all_stakers {
        let mut staker_info = STAKER_INFO.load(deps.storage, (lp_id.into(), &staker))?;
        let staker_share = Decimal::from_ratio(staker_info.amt_staked, vault_share);

        // generator rewards
        if pending_gen_rewards.pending > Uint128::zero() {
            staker_info.rewards.generator_rewards[0].amount += staker_share * pending_gen_rewards.pending;
        }
        if pending_proxy > Uint128::zero() {
            staker_info.rewards.generator_rewards[1].amount += staker_share * pending_proxy;
        }

        // amm rewards
        if pending_amm_rewards[0].amount > Uint128::zero() {
            staker_info.rewards.amm_rewards[0].amount += staker_share * pending_amm_rewards[0].amount;
        }
        if pending_amm_rewards[0].amount > Uint128::zero() {
            staker_info.rewards.amm_rewards[1].amount += staker_share * pending_amm_rewards[1].amount;
        }

        // save new reward info
        STAKER_INFO.save(deps.storage, (lp_id.into(), &staker), &staker_info)?;
    }

    // save new liquidity
    lp_info.last_liquidity = new_liquidity;
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info);
    
    Ok(Response::new().add_messages(messages))
}