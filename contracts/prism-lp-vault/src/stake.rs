#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, to_binary, Addr, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, Order, Response,
    StdError, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use prism_protocol::collector::ExecuteMsg as PrismCollectorExecuteMsg;
use prism_protocol::lp_vault::{Config, ExecuteMsg, RewardInfo, StakerInfo, StakingMode};
use prism_common::decimal_division;

use astroport::asset::{Asset, AssetInfo};
use astroport::generator::{ExecuteMsg as AstroGenExecuteMsg, PendingTokenResponse};
use astroport::pair::Cw20HookMsg as AstroPairHookMsg;

use crate::query::{
    query_generator_rewards, query_lp_burn_rewards, query_pair_info,
    query_pending_generator_rewards, query_pool_info,
};
use crate::state::{CONFIG, LP_IDS, LP_INFOS, STAKER_INFO};

pub fn stake(
    deps: DepsMut,
    env: Env,
    token: Addr,
    sender_addr: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    if amount <= Uint128::zero() {
        return Err(StdError::generic_err("invalid staking amount"));
    }

    // check if LP token exists and is a proper yLP token
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    if token != lp_info.ylp_contract {
        return Err(StdError::generic_err("token sent is not a yLP token"));
    }

    let messages = vec![
        // update rewards for this LP token
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::UpdateLPRewards {
                token: lp_info.lp_contract,
            })?,
            funds: vec![],
        }),
        // send user their pending rewards
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::SendStakerRewards {
                staker: sender_addr.clone(),
            })?,
            funds: vec![],
        }),
        // update internal state
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::UpdateStakerInfo {
                lp_id,
                sender_addr: sender_addr.clone(),
                amount,
                stake: true,
            })?,
            funds: vec![],
        }),
    ];

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "stake"),
        attr("from", sender_addr.as_str()),
        attr("LP", token.as_str()),
        attr("amount", amount),
    ]))
}

pub fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    // check if LP token exists and is a proper yLP token
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No LP address exists".to_string()))?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    if token != lp_info.ylp_contract {
        return Err(StdError::generic_err("token sent is not a yLP token"));
    }

    // check if staker is valid
    let sender_addr = deps.api.addr_validate(info.sender.as_str())?;
    let stake = STAKER_INFO.may_load(deps.storage, (lp_id.into(), &sender_addr))?;
    if stake == None {
        return Err(StdError::generic_err("invalid staker"));
    }
    let stake_info = stake.unwrap();

    // check if amount is valid
    let unstake_amt = match amount {
        Some(stake) => stake,
        None => stake_info.amt_staked,
    };
    if unstake_amt > stake_info.amt_staked {
        return Err(StdError::generic_err("invalid staking amount"));
    }

    let messages = vec![
        // update rewards for this token
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::UpdateLPRewards {
                token: lp_info.lp_contract.clone(),
            })?,
            funds: vec![],
        }),
        // send user their pending rewards
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::SendStakerRewards {
                staker: sender_addr.clone(),
            })?,
            funds: vec![],
        }),
        // transfer yLP back to user
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: lp_info.ylp_contract.into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender_addr.to_string(),
                amount: unstake_amt,
            })?,
            funds: vec![],
        }),
        // update internal state
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::UpdateStakerInfo {
                lp_id,
                sender_addr: sender_addr.clone(),
                amount: unstake_amt,
                stake: false,
            })?,
            funds: vec![],
        }),
    ];

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "unstake"),
        attr("from", sender_addr.as_str()),
        attr("LP", token.as_str()),
        attr("amount", unstake_amt),
    ]))
}

// QUES: This feels pretty inefficient, is it enough of an amount to matter?
pub fn claim_rewards(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let staker = deps.api.addr_validate(info.sender.as_str())?;

    // update rewards of all relevant LP tokens
    let mut messages: Vec<CosmosMsg> = STAKER_INFO
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (user, staker_info) = item.unwrap();
            if staker
                == deps
                    .api
                    .addr_validate(&String::from_utf8(user).unwrap())
                    .unwrap()
            {
                Some(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone().to_string(),
                    msg: to_binary(&ExecuteMsg::UpdateLPRewards {
                        token: staker_info.lp_contract,
                    })
                    .ok()?,
                    funds: vec![],
                }))
            } else {
                None
            }
        })
        .collect();

    // send all rewards to staker
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::SendStakerRewards {
            staker: staker.clone(),
        })?,
        funds: vec![],
    }));

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "claim rewards"),
        attr("from", staker.as_str()),
    ]))
}

pub fn update_staking_mode(
    deps: DepsMut,
    info: MessageInfo,
    token: Addr,
    mode: StakingMode,
) -> StdResult<Response> {
    let lp_id = LP_IDS
        .load(deps.storage, &token)
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;

    STAKER_INFO
        .update(
            deps.storage,
            (lp_id.into(), &info.sender),
            |stake| -> StdResult<StakerInfo> {
                let mut stake_info = stake.unwrap();
                stake_info.mode = mode;
                Ok(stake_info)
            },
        )
        .map_err(|_| StdError::generic_err("No LP token exists".to_string()))?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update staking mode"),
        attr("from", info.sender.as_str()),
        attr("LP", token.as_str()),
    ]))
}

// QUES: should i break this up?
pub fn update_lp_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: Addr,
) -> StdResult<Response> {
    if info.sender.as_str() != env.contract.address {
        return Err(StdError::generic_err("only callable by contract"));
    }

    let config: Config = CONFIG.load(deps.storage)?;
    let lp_id = LP_IDS.load(deps.storage, &token)?;
    let mut lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;
    let vault_share = lp_info.amt_bonded;

    // claim astro generator rewards
    let mut pending_gen_rewards: PendingTokenResponse = query_pending_generator_rewards(
        deps.as_ref(),
        env,
        &deps.querier,
        lp_info.lp_contract.clone(),
    )?;
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

    // QUES: how to do this better
    // why is math so hard to do between Decimal and Uint128
    // s = liquidity per token = sqrt(xy)/number of LP
    // withdraw and burn (1 - s_last/s_new)*vault_share of LP tokens
    let s = Decimal::from_ratio(
        pool_info.assets[0].amount * pool_info.assets[1].amount,
        Uint128::new(1),
    )
    .sqrt();
    let new_liquidity: Decimal = s / pool_info.total_share;
    let inv_new_liquidity = decimal_division(Uint128::new(1), new_liquidity);
    let inv_last_liquidity = decimal_division(Uint128::new(1), lp_info.last_liquidity);
    let tokens_to_burn =
        (Uint128::new(1).checked_sub(inv_new_liquidity / inv_last_liquidity)?) * lp_info.amt_bonded;

    let mut pending_amm_rewards: Vec<Asset> = query_lp_burn_rewards(
        deps.as_ref(),
        &deps.querier,
        lp_info.lp_contract.clone(),
        tokens_to_burn,
    )?;

    if pending_amm_rewards[0].amount > Uint128::zero()
        || pending_amm_rewards[1].amount > Uint128::zero()
    {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token.into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: lp_info.pair_contract.clone().into_string(),
                msg: to_binary(&AstroPairHookMsg::WithdrawLiquidity {})?,
                amount: tokens_to_burn,
            })?,
            funds: vec![],
        }));
    }

    // exit early if no pending rewards
    if !(pending_gen_rewards.pending > Uint128::zero()
        || pending_proxy > Uint128::zero()
        || pending_amm_rewards[0].amount > Uint128::zero()
        || pending_amm_rewards[1].amount > Uint128::zero())
    {
        return Ok(Response::new());
    }

    // deduct fees and send to collector
    // 15% of accrued AMM and generator rewards go to PRISM holders
    // the share of reward accrued from unstaked yLP tokens goes to PRISM holders as well, and is calculated later

    // ASTRO reward from generator
    let mut prism_fees: Vec<Uint128> = vec![Uint128::zero(); 4];
    if pending_gen_rewards.pending > Uint128::zero() {
        let prism_fee = pending_gen_rewards.pending * config.fee;
        prism_fees[0] += prism_fee;
        pending_gen_rewards.pending = pending_gen_rewards.pending.checked_sub(prism_fee)?;
    }

    // proxy reward from generator
    if pending_proxy > Uint128::zero() {
        let prism_fee = pending_proxy * config.fee;
        prism_fees[1] += prism_fee;
        pending_proxy = pending_proxy.checked_sub(prism_fee)?;
    }

    // first underlying AMM reward
    if pending_amm_rewards[0].amount > Uint128::zero() {
        let prism_fee = pending_amm_rewards[0].clone().amount * config.fee;
        prism_fees[2] += prism_fee;
        pending_amm_rewards[0].amount = pending_amm_rewards[0].amount.checked_sub(prism_fee)?;
    }

    // second underlying AMM reward
    if pending_amm_rewards[1].amount > Uint128::zero() {
        let prism_fee = pending_amm_rewards[1].clone().amount * config.fee;
        prism_fees[3] += prism_fee;
        pending_amm_rewards[1].amount = pending_amm_rewards[1].amount.checked_sub(prism_fee)?;
    }

    // get all stakers infos for this LP
    let all_stakers: Vec<Addr> = STAKER_INFO
        .prefix(lp_id.into())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (staker, _) = item.unwrap();
            deps.api
                .addr_validate(&String::from_utf8(staker).unwrap())
                .unwrap()
        })
        .collect();

    // update reward infos
    let mut distributed_rewards: Vec<Uint128> = vec![Uint128::zero(); 4];
    for staker in all_stakers {
        let mut staker_info = STAKER_INFO.load(deps.storage, (lp_id.into(), &staker))?;
        let staker_share = Decimal::from_ratio(staker_info.amt_staked, vault_share);

        // generator rewards
        if pending_gen_rewards.pending > Uint128::zero() {
            let reward_share = staker_share * pending_gen_rewards.pending;
            staker_info.rewards.generator_rewards[0].amount += reward_share;
            distributed_rewards[0] += reward_share;
        }
        if pending_proxy > Uint128::zero() {
            let reward_share = staker_share * pending_proxy;
            staker_info.rewards.generator_rewards[1].amount += reward_share;
            distributed_rewards[1] += reward_share;
        }

        // amm rewards
        if pending_amm_rewards[0].amount > Uint128::zero() {
            let reward_share = staker_share * pending_amm_rewards[0].amount;
            staker_info.rewards.amm_rewards[0].amount += reward_share;
            distributed_rewards[2] += reward_share;
        }
        if pending_amm_rewards[0].amount > Uint128::zero() {
            let reward_share = staker_share * pending_amm_rewards[1].amount;
            staker_info.rewards.amm_rewards[1].amount += reward_share;
            distributed_rewards[3] += reward_share;
        }

        // save new reward info
        STAKER_INFO.save(deps.storage, (lp_id.into(), &staker), &staker_info)?;
    }

    // add unclaimed rewards to fee
    let unclaimed_rewards = vec![
        pending_gen_rewards
            .pending
            .checked_sub(distributed_rewards[0]),
        pending_proxy.checked_sub(distributed_rewards[1]),
        pending_amm_rewards[0]
            .amount
            .checked_sub(distributed_rewards[2]),
        pending_amm_rewards[1]
            .amount
            .checked_sub(distributed_rewards[3]),
    ];

    for (i, unclaimed_fees) in unclaimed_rewards.iter().enumerate() {
        match unclaimed_fees {
            Ok(fee) => {
                prism_fees[i] += fee;
            }
            Err(_) => {}
        }
    }

    // send fees to collector
    // ASTRO reward from generator
    if prism_fees[0] > Uint128::zero() {
        let reward_asset = Asset {
            info: lp_info.generator_reward_info[0].clone(),
            amount: prism_fees[0],
        };
        messages.push(
            reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector)?)?,
        );
    }

    // proxy reward from generator
    if prism_fees[1] > Uint128::zero() {
        let reward_asset = Asset {
            info: lp_info.generator_reward_info[1].clone(),
            amount: prism_fees[1],
        };
        messages.push(
            reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector)?)?,
        );
    }

    // first underlying AMM reward
    if prism_fees[2] > Uint128::zero() {
        let reward_asset = Asset {
            info: pending_amm_rewards[0].clone().info,
            amount: prism_fees[2],
        };
        messages.push(
            reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector)?)?,
        );
    }

    // second underlying AMM reward
    if prism_fees[3] > Uint128::zero() {
        let reward_asset = Asset {
            info: pending_amm_rewards[1].clone().info,
            amount: prism_fees[3],
        };
        messages.push(
            reward_asset.into_msg(&deps.querier, deps.api.addr_validate(&config.collector)?)?,
        );
    }

    // save new liquidity
    lp_info.last_liquidity = new_liquidity;
    LP_INFOS.save(deps.storage, lp_id.into(), &lp_info)?;

    Ok(Response::new().add_messages(messages))
}

// QUES: This feels pretty inefficient, is it enough of an amount to matter?
pub fn send_staker_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staker: Addr,
) -> StdResult<Response> {
    if info.sender.as_str() != env.contract.address {
        return Err(StdError::generic_err("only callable by contract"));
    }

    let config: Config = CONFIG.load(deps.storage)?;

    // get all LP's for this staker
    let lp_ids: Vec<u64> = STAKER_INFO
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (user, staker_info) = item.unwrap();
            if staker
                == deps
                    .api
                    .addr_validate(&String::from_utf8(user).unwrap())
                    .unwrap()
            {
                Some(LP_IDS.load(deps.storage, &staker_info.lp_contract).unwrap())
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
            }
            // QUES: (for olly) sends PRISM and not XPRISM, this is correct right?
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
                        assets,
                        receiver: Some(staker.to_string()),
                    })?,
                    funds: vec![],
                }));
            }
            StakingMode::Autocompound => {
                // WIP
            }
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

pub fn update_staker_info(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_id: u64,
    sender_addr: Addr,
    amount: Uint128,
    stake: bool,
) -> StdResult<Response> {
    if info.sender.as_str() != env.contract.address {
        return Err(StdError::generic_err("only callable by contract"));
    }

    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;
    if stake {
        let stake_info = match STAKER_INFO.load(deps.storage, (lp_id.into(), &sender_addr)) {
            Ok(mut info) => {
                info.amt_staked += amount;
                info
            }
            // create new staker info if it doesn't exist
            // QUES: Is there some cleaner way to do all this?
            Err(_) => {
                // grab generator reward info
                let generator_info = query_generator_rewards(
                    deps.as_ref(),
                    &deps.querier,
                    lp_info.lp_contract.clone(),
                )?;

                let mut generator_rewards = vec![
                    Asset {
                        info: generator_info[0].clone(),
                        amount: Uint128::zero(),
                    },
                    Asset {
                        // we have a placeholder asset if proxy doesn't exist
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked(""),
                        },
                        amount: Uint128::zero(),
                    },
                ];
                // add proxy reward info if it exists
                if generator_info.len() > 1 {
                    generator_rewards[1].info = generator_info[1].clone();
                    generator_rewards[1].amount = Uint128::zero();
                }

                // grab amm reward info
                let amm_info =
                    query_pair_info(deps.as_ref(), &deps.querier, lp_info.lp_contract.clone())?;
                let amm_rewards = vec![
                    Asset {
                        info: amm_info.asset_infos[0].clone(),
                        amount: Uint128::zero(),
                    },
                    Asset {
                        info: amm_info.asset_infos[1].clone(),
                        amount: Uint128::zero(),
                    },
                ];

                StakerInfo {
                    lp_contract: lp_info.lp_contract,
                    amt_staked: amount,
                    mode: StakingMode::Default,
                    rewards: RewardInfo {
                        generator_rewards,
                        amm_rewards,
                    },
                }
            }
        };
        STAKER_INFO.save(deps.storage, (lp_id.into(), &sender_addr), &stake_info)?;
    } else {
        STAKER_INFO.update(
            deps.storage,
            (lp_id.into(), &sender_addr),
            |stake| -> StdResult<StakerInfo> {
                let mut stake_info = stake.unwrap();
                stake_info.amt_staked = stake_info.amt_staked.checked_sub(amount)?;
                Ok(stake_info)
            },
        )?;
    }

    Ok(Response::new())
}
