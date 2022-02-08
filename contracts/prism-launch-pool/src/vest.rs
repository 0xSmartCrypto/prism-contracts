use crate::contract::{pull_pending_rewards, update_reward_index};
use crate::error::ContractError;
use crate::state::{CONFIG, PENDING_WITHDRAW, REWARD_INFO, SCHEDULED_VEST};
use cosmwasm_std::{DepsMut, Env, MessageInfo, Order, Response, StdResult, Storage, Uint128};
use cw_asset::{Asset, AssetInfo};
use std::convert::TryInto;

// seconds in a day, make time discrete per day
pub const TIME_UNIT: u64 = 60 * 60 * 24;
pub const REDEMPTION_TIME: u64 = TIME_UNIT * 21u64;

pub fn update_vest(storage: &mut dyn Storage, current_time: u64, address: &str) -> StdResult<()> {
    let mut can_withdraw = PENDING_WITHDRAW
        .load(storage, address.as_bytes())
        .unwrap_or_else(|_| Uint128::zero());
    let mut to_delete = vec![];

    for item in
        SCHEDULED_VEST
            .prefix(address.as_bytes())
            .range(storage, None, None, Order::Ascending)
    {
        let (key, unlocked) = item?;
        let end_time = u64::from_be_bytes(key.try_into().unwrap());
        if current_time < end_time {
            break;
        }
        can_withdraw += unlocked;
        to_delete.push(end_time);
    }

    for t in to_delete {
        SCHEDULED_VEST.remove(storage, (address.as_bytes(), &t.to_be_bytes()))
    }
    PENDING_WITHDRAW.save(storage, address.as_bytes(), &can_withdraw)
}

pub fn withdraw_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    update_reward_index(deps.storage, &env)?;
    pull_pending_rewards(deps.storage, &info.sender.clone().to_string())?;

    let mut reward_info = REWARD_INFO.load(deps.storage, info.sender.as_bytes())?;
    let to_withdraw = reward_info.pending_reward;
    reward_info.pending_reward = Uint128::zero();
    REWARD_INFO.save(deps.storage, info.sender.as_bytes(), &reward_info)?;
    update_vest(deps.storage, env.block.time.seconds(), info.sender.as_str())?;

    if !to_withdraw.is_zero() {
        let mut end_time = env.block.time.seconds() + REDEMPTION_TIME;
        end_time -= end_time % TIME_UNIT;

        let orig_vest = SCHEDULED_VEST
            .load(
                deps.storage,
                (info.sender.as_bytes(), &end_time.to_be_bytes()),
            )
            .unwrap_or_else(|_| Uint128::zero());
        SCHEDULED_VEST.save(
            deps.storage,
            (info.sender.as_bytes(), &end_time.to_be_bytes()),
            &(orig_vest + to_withdraw),
        )?;
    }
    Ok(Response::new().add_attribute("withdraw_amount", to_withdraw.to_string()))
}

pub fn claim_withdrawn_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    update_vest(deps.storage, env.block.time.seconds(), info.sender.as_str())?;
    let amount = PENDING_WITHDRAW.load(deps.storage, info.sender.to_string().as_bytes())?;
    if amount.is_zero() {
        return Err(ContractError::InvalidClaimWithdrawnRewards {
            reason: "There are no claimable rewards".to_string(),
        });
    }

    PENDING_WITHDRAW.save(
        deps.storage,
        info.sender.to_string().as_bytes(),
        &Uint128::zero(),
    )?;
    let to_withdraw = Asset {
        info: AssetInfo::Cw20(cfg.prism_token),
        amount,
    };
    Ok(Response::new().add_message(to_withdraw.transfer_msg(info.sender)?))
}
