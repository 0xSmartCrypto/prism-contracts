use crate::contract::{pull_pending_rewards, update_reward_index};
use crate::state::{CONFIG, PENDING_WITHDRAW, REWARD_INFO, SCHEDULED_VEST};
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult, Storage, Uint128};
use cw_storage_plus::Bound;
use prism_protocol::common::OrderBy;
use std::convert::TryInto;
use astroport::asset::{Asset, AssetInfo};

// seconds in a day, make time discrete per day
pub const TIME_UNIT: u64 = 60 * 60 * 24;
pub const REDEMPTION_TIME: u64 = TIME_UNIT * 21u64;

pub fn update_vest(
    storage: &mut dyn Storage,
    current_time: u64,
    address: &String,
) -> StdResult<()> {
    let start = Some(Bound::Inclusive(address.as_bytes().to_vec()));
    let address_vec = address.as_bytes().to_vec();
    let address_len = address_vec.len();

    let mut can_withdraw = PENDING_WITHDRAW
        .load(storage, address.as_bytes())
        .unwrap_or(Uint128::zero());
    let mut to_delete = vec![];

    for item in SCHEDULED_VEST.range(storage, start, None, OrderBy::Asc.into()) {
        let (key, unlocked) = item?;
        let end_time = u64::from_be_bytes(key[address_len..].try_into().unwrap());
        if !key.starts_with(address_vec.as_slice()) || current_time < end_time {
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

pub fn withdraw_rewards(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    update_reward_index(deps.storage, &env)?;
    pull_pending_rewards(deps.storage, &info.sender.clone().to_string())?;

    let mut reward_info = REWARD_INFO.load(deps.storage, info.sender.as_bytes())?;
    reward_info.pending_reward = Uint128::zero();
    REWARD_INFO.save(deps.storage, info.sender.as_bytes(), &reward_info)?;
    update_vest(
        deps.storage,
        env.block.time.seconds(),
        &info.sender.to_string(),
    )?;

    let mut end_time = env.block.time.seconds() + REDEMPTION_TIME;
    end_time -= end_time % TIME_UNIT;

    let orig_vest = SCHEDULED_VEST
        .load(deps.storage, (info.sender.as_bytes(), &end_time.to_be_bytes()))
        .unwrap_or(Uint128::zero());

    SCHEDULED_VEST.save(
        deps.storage,
        (info.sender.as_bytes(), &end_time.to_be_bytes()),
        &(orig_vest + reward_info.pending_reward),
    )?;
    Ok(Response::new())
}

pub fn claim_withdrawn_rewards(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    update_vest(
        deps.storage,
        env.block.time.seconds(),
        &info.sender.to_string(),
    )?;
    let amount = PENDING_WITHDRAW.load(deps.storage, info.sender.to_string().as_bytes())?;
    PENDING_WITHDRAW.save(
        deps.storage,
        info.sender.to_string().as_bytes(),
        &Uint128::zero(),
    )?;
    let to_withdraw = Asset {
        info: AssetInfo::Token {
            contract_addr: cfg.prism_token.clone(),
        },
        amount,
    };
    Ok(Response::new().add_message(to_withdraw.into_msg(&deps.querier, info.sender)?))
}
