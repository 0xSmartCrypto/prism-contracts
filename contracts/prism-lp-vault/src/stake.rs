#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, SubMsg, attr, Addr, CanonicalAddr, CosmosMsg, WasmMsg, Reply, ReplyOn, Decimal, Order, Storage
};

use prism_protocol::lp_vault::{
    ConfigResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode, 
};

use prism_protocol::collector::{ExecuteMsg as PrismCollectorExecuteMsg};

use astroport::asset::{AssetInfo, Asset, addr_validate_to_lower};
use astroport::generator::{ExecuteMsg as AstroGenExecuteMsg, PendingTokenResponse};
use astroport::pair::{Cw20HookMsg as AstroPairHookMsg};
use astroport::token::{InstantiateMsg as AstroTokenInstantiateMsg};
use astroport::factory::{ConfigResponse as FactoryConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS, NUM_LPS, LPInfo, STAKER_INFO, StakerInfo, RewardInfo};
use crate::query::{query_config, query_token_info, query_pair_info, query_factory_config, query_pending_generator_rewards, query_pool_info, query_lp_burn_rewards, query_generator_rewards};
use crate::math::decimal_division;

use crate::response::MsgInstantiateContractResponse;
use protobuf::Message;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, TokenInfoResponse, MinterResponse};
use terra_cosmwasm::TerraMsgWrapper;

// only callable by cw20
pub fn stake(
    deps: DepsMut,
    env: Env,
    staking_token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    if !(amount > Uint128::zero()) {
        return Err(StdError::generic_err("invalid staking amount"));
    }

    // check if LP token exists and is a proper yLP token
    let lp_id = LP_IDS.load(deps.storage, &staking_token.clone())
        .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    if staking_token != lp_info.ylp_contract {
        return Err(StdError::generic_err("token sent is not a yLP token"));
    }

    // update rewards for this token
    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::UpdateLPRewards {
            token: staking_token.clone(),
        })?,
        funds: vec![],
    }));

    // send user their pending rewards
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::SendStakerRewards {
            staker: sender_addr.clone(),
        })?,
        funds: vec![],
    }));

    // create new staker info if it doesn't exist
    let stake_info = match STAKER_INFO.load(deps.storage, (lp_id.into(), &sender_addr.clone())) {
        Ok(mut info) => {
            info.amt_staked += amount;
            info
        },
        Err(_) => {
            let generator_info = query_generator_rewards(deps.as_ref(), &deps.querier, lp_info.lp_contract.clone())?;
            let mut generator_rewards = vec![
                Asset {
                    info: generator_info[0].clone(),
                    amount: Uint128::zero(),
                },
                Asset {
                    // we have a placeholder asset if proxy doesn't exist
                    info: AssetInfo::Token { contract_addr: Addr::unchecked("") },
                    amount: Uint128::zero(),
                },
            ];
            // add proxy reward if it exists
            if generator_info.len() > 1 {
                generator_rewards[1].info = generator_info[1].clone();
                generator_rewards[1].amount = Uint128::zero();
            }

            // grab amm reward info
            let amm_info = query_pair_info(deps.as_ref(), &deps.querier, lp_info.lp_contract.clone())?;
            let amm_rewards = vec![
                Asset {
                    info: amm_info.asset_infos[0].clone(),
                    amount: Uint128::zero(),
                },
                Asset {
                    info: amm_info.asset_infos[1].clone(),
                    amount: Uint128::zero(),
                }
            ];

            let new_stake = StakerInfo {
                lp_addr: lp_info.lp_contract,
                amt_staked: amount,
                mode: StakingMode::Default,
                rewards: RewardInfo {
                    generator_rewards: generator_rewards,
                    amm_rewards: amm_rewards,
                }
            };

            new_stake
        }
    };

    STAKER_INFO.save(deps.storage, (lp_id.into(), &sender_addr.clone()), &stake_info)?;
    Ok(Response::new().add_messages(messages))
}

// ??
// rewrite all of this garbage
pub fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    // check if LP token exists and is a proper yLP token
    let lp_id = LP_IDS.load(deps.storage, &token.clone())
        .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    if token != lp_info.ylp_contract {
        return Err(StdError::generic_err("token sent is not a yLP token"));
    }

    let sender_addr = deps.api.addr_validate(info.sender.as_str())?;
    
    let mut stake_info = match STAKER_INFO.load(deps.storage, (lp_id.into(), &sender_addr.clone())) {
        Ok(mut info) => {
            Some(info)
        },
        Err(_) => {
            None
        },
    };

    // get correct values and throw relevant errors
    if stake_info == None {
        return Err(StdError::generic_err("invalid staker"));
    }

    let mut unwrapped_stake_info = stake_info.unwrap();

    let unstake_amt = match amount {
        Some(stake) => {
            stake
        },
        None => {
            unwrapped_stake_info.clone().amt_staked
        }
    };

    if unstake_amt > unwrapped_stake_info.amt_staked {
        return Err(StdError::generic_err("invalid staking amount"));
    }

    // update rewards for this token
    let mut messages: Vec<CosmosMsg> = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::UpdateLPRewards {
            token: lp_info.lp_contract.clone(),
        })?,
        funds: vec![],
    }));

    // send user their pending rewards
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::SendStakerRewards {
            staker: sender_addr.clone(),
        })?,
        funds: vec![],
    }));

    // initiate transfer
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.ylp_contract.clone().into_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: sender_addr.clone().to_string(),
            amount: unstake_amt,
        })?,
        funds: vec![],
    }));

    // update staker info
    unwrapped_stake_info.amt_staked = unwrapped_stake_info.amt_staked.checked_sub(unstake_amt)?;
    STAKER_INFO.save(deps.storage, (lp_id.into(), &sender_addr), &unwrapped_stake_info)?;

    Ok(Response::new().add_messages(messages))
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    // feels kinda pricy
    let staker = deps.api.addr_validate(info.sender.as_str())?;

    // update rewards of all relevant LP tokens
    let mut messages: Vec<CosmosMsg> = STAKER_INFO
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (user, staker_info) = item.unwrap();
            if staker == deps.api.addr_validate(&String::from_utf8(user).unwrap()).unwrap() {
                Some(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone().to_string(),
                    msg: to_binary(&ExecuteMsg::UpdateLPRewards {
                        token: staker_info.lp_addr.clone(),
                    }).ok()?,
                    funds: vec![],
                }))
            } else {
                None
            }
        })
        .collect();

    // send all rewards to staker
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::SendStakerRewards {
            staker: staker.clone(),
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages))
}

pub fn update_staking_mode(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
    mode: StakingMode,
) -> StdResult<Response> {
    let lp_id = LP_IDS.load(deps.storage, &token.clone())
                            .map_err(|_| StdError::generic_err(format!("No LP address exists")))?;
    let mut stake_info = STAKER_INFO.load(deps.storage, (lp_id.into(), &info.sender.clone()))
                            .map_err(|_| StdError::generic_err(format!("Staker does not exist for this LP")))?;
    
    let mut messages = vec![];
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::UpdateLPRewards {
            token: token.clone(),
        })?,
        funds: vec![],
    }));

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().to_string(),
        msg: to_binary(&ExecuteMsg::SendStakerRewards {
            staker: deps.api.addr_validate(info.sender.as_str())?,
        })?,
        funds: vec![],
    }));

    // update StakingMode
    stake_info.mode = mode;
    STAKER_INFO.save(deps.storage, (lp_id.into(), &info.sender.clone()), &stake_info)?;
    Ok(Response::new().add_messages(messages))
}

pub fn update_lp_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
) -> StdResult<Response> {
    if info.sender.as_str() != env.contract.address.to_string() {
        return Err(StdError::generic_err("only callable by contract"));
    }

    let config: Config = CONFIG.load(deps.storage)?;
    let lp_id = LP_IDS.load(deps.storage, &token.clone())?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;
    let vault_share = lp_info.amt_bonded.clone();

    // calculate and withdraw astro generator rewards
    let mut pending_gen_rewards: PendingTokenResponse = query_pending_generator_rewards(deps.as_ref(), env, &deps.querier, lp_info.lp_contract.clone())?;
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
    let pool_info = query_pool_info(deps.as_ref(), &deps.querier, lp_info.lp_contract.clone())?;

    // why is math so hard to do here
    // s = liquidity per token = sqrt(xy)/number of LP
    // withdraw and burn (1 - s_last/s_new)*vault_share of LP tokens
    let s = Decimal::from_ratio(pool_info.assets[0].amount * pool_info.assets[1].amount, Uint128::new(1)).sqrt();
    let new_liquidity: Decimal = s / pool_info.total_share;
    let inv_new_liquidity = decimal_division(Uint128::new(1), new_liquidity);
    let inv_last_liquidity = decimal_division(Uint128::new(1), lp_info.last_liquidity);
    let tokens_to_burn = (Uint128::new(1).checked_sub(inv_new_liquidity / inv_last_liquidity)?) * lp_info.amt_bonded;

    let mut pending_amm_rewards: Vec<Asset> = query_lp_burn_rewards(deps.as_ref(), &deps.querier, lp_info.lp_contract.clone(), tokens_to_burn.clone())?;

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

    // deduct fees and send to collector
    // ASTRO reward from generator
    if pending_gen_rewards.pending > Uint128::zero() {
        let prism_fee = pending_gen_rewards.pending * config.fee;
        let reward_asset = Asset {
            info: lp_info.generator_reward_info[0].clone(),
            amount: prism_fee
        };
        pending_gen_rewards.pending = pending_gen_rewards.pending.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector.clone())?)?);
    }

    // proxy reward from generator
    if pending_proxy > Uint128::zero() {
        let prism_fee = pending_proxy * config.fee;
        let reward_asset = Asset {
            info: lp_info.generator_reward_info[1].clone(),
            amount: prism_fee
        };
        pending_proxy = pending_proxy.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector.clone())?)?);
    }

    // first underlying AMM reward
    if pending_amm_rewards[0].amount > Uint128::zero() {
        let prism_fee = pending_amm_rewards[0].clone().amount * config.fee;
        let reward_asset = Asset {
            info: pending_amm_rewards[0].clone().info,
            amount: prism_fee,
        };
        pending_amm_rewards[0].amount = pending_amm_rewards[0].amount.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector.clone())?)?);
    }

    // second underlying AMM reward
    if pending_amm_rewards[1].amount > Uint128::zero() {
        let prism_fee = pending_amm_rewards[1].clone().amount * config.fee;
        let reward_asset = Asset {
            info: pending_amm_rewards[1].clone().info,
            amount: prism_fee,
        };
        pending_amm_rewards[1].amount = pending_amm_rewards[1].amount.checked_sub(prism_fee)?;
        messages.push(reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector.clone())?)?);
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
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info)?;
    
    Ok(Response::new().add_messages(messages))
}

pub fn send_staker_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staker: Addr,
) -> StdResult<Response> {
    if info.sender.as_str() != env.contract.address.to_string() {
        return Err(StdError::generic_err("only callable by contract"));
    }

    let config: Config = CONFIG.load(deps.storage)?;
    // find some better way to do this, seems wasteful
    
    // get all LP's for this staker
    let lp_ids: Vec<u64> = STAKER_INFO
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (user, staker_info) = item.unwrap();
            if staker == deps.api.addr_validate(&String::from_utf8(user).unwrap()).unwrap() {
                Some(LP_IDS.load(deps.storage, &staker_info.lp_addr).unwrap())
            } else {
                None
            }
        })
        .collect();

    // send staker relevant rewards
    let mut messages = vec![];
    for lp in lp_ids {
        let mut stake_info = STAKER_INFO.load(deps.storage, (lp.into(), &staker.clone()))?;

        match stake_info.mode {
            StakingMode::Default => {
                let astro_reward = stake_info.rewards.generator_rewards[0].clone();
                if astro_reward.amount > Uint128::zero() {
                    stake_info.rewards.generator_rewards[0].amount = Uint128::zero();
                    messages.push(astro_reward.into_msg(&deps.querier, staker.clone())?);
                }
                
                let proxy_reward = stake_info.rewards.generator_rewards[1].clone();
                if proxy_reward.amount > Uint128::zero() {
                    stake_info.rewards.generator_rewards[1].amount = Uint128::zero();
                    messages.push(proxy_reward.into_msg(&deps.querier, staker.clone())?);
                }

                let amm1_reward = stake_info.rewards.amm_rewards[0].clone();
                if amm1_reward.amount > Uint128::zero() {
                    stake_info.rewards.amm_rewards[0].amount = Uint128::zero();
                    messages.push(amm1_reward.into_msg(&deps.querier, staker.clone())?);
                }

                let amm2_reward = stake_info.rewards.amm_rewards[1].clone();
                if amm2_reward.amount > Uint128::zero() {
                    stake_info.rewards.amm_rewards[1].amount = Uint128::zero();
                    messages.push(amm2_reward.into_msg(&deps.querier, staker.clone())?);
                }
            },
            StakingMode::XPrism => {
                let mut assets: Vec<Asset> = vec![];
                let astro_reward = stake_info.rewards.generator_rewards[0].clone();
                if astro_reward.amount > Uint128::zero() {
                    stake_info.rewards.generator_rewards[0].amount = Uint128::zero();
                    assets.push(astro_reward);
                }
                
                let proxy_reward = stake_info.rewards.generator_rewards[1].clone();
                if proxy_reward.amount > Uint128::zero() {
                    stake_info.rewards.generator_rewards[1].amount = Uint128::zero();
                    assets.push(proxy_reward);
                }

                let amm1_reward = stake_info.rewards.amm_rewards[0].clone();
                if amm1_reward.amount > Uint128::zero() {
                    stake_info.rewards.amm_rewards[0].amount = Uint128::zero();
                    assets.push(amm1_reward);
                }

                let amm2_reward = stake_info.rewards.amm_rewards[1].clone();
                if amm2_reward.amount > Uint128::zero() {
                    stake_info.rewards.amm_rewards[1].amount = Uint128::zero();
                    assets.push(amm2_reward);
                }

                // convert rewards to prism and send to user
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: config.collector.clone().to_string(),
                    msg: to_binary(&PrismCollectorExecuteMsg::ConvertAndSend {
                        assets: assets,
                        receiver: Some(staker.to_string()),
                    })?,
                    funds: vec![],
                }));
            },
            StakingMode::Autocompound => {
                // WIP
            },
        };

        // save new stake info if theres still a stake, else delete
        if stake_info.amt_staked == Uint128::zero() {
            STAKER_INFO.remove(deps.storage, (lp.into(), &staker.clone()));
        } else {
            STAKER_INFO.save(deps.storage, (lp.into(), &staker.clone()), &stake_info)?;
        }
    }

    Ok(Response::new().add_messages(messages))
}