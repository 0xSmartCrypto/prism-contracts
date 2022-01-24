use crate::state::{
    bank_read, bank_store, config_store, poll_read, poll_voter_store, Config, Poll,
    VotingTokenManager,
};

use cosmwasm_std::{
    attr, to_binary, CanonicalAddr, CosmosMsg, Deps, DepsMut, MessageInfo, Response, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::gov::{PollStatus, VotingTokensResponse};

pub fn stake_voting_tokens(deps: DepsMut, sender: String, amount: Uint128) -> StdResult<Response> {
    if amount.is_zero() {
        return Err(StdError::generic_err("Insufficient funds sent"));
    }

    let sender_address_raw = deps.api.addr_canonicalize(&sender)?;
    let key = &sender_address_raw.as_slice();

    let mut token_manager = bank_read(deps.storage).may_load(key)?.unwrap_or_default();

    token_manager.deposit += amount;

    bank_store(deps.storage).save(key, &token_manager)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "stake_voting_tokens"),
        attr("sender", sender.as_str()),
        attr("amount", amount.to_string()),
    ]))
}

// Withdraw amount if not staked. By default all funds will be withdrawn.
pub fn withdraw_voting_tokens(
    deps: DepsMut,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    let config: Config = config_store(deps.storage).load()?;
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let key = sender_address_raw.as_slice();

    let mut token_manager = bank_read(deps.storage)
        .load(key)
        .map_err(|_| StdError::generic_err("no voting information found for this address"))?;

    let user_locked_balance =
        compute_locked_balance(deps.storage, &mut token_manager, &sender_address_raw)?;

    let withdrawable_balance = token_manager.deposit.checked_sub(user_locked_balance)?;
    let withdraw_amount = amount.unwrap_or(withdrawable_balance);

    if withdraw_amount > withdrawable_balance {
        return Err(StdError::generic_err(
            "User is trying to withdraw too many tokens",
        ));
    }

    token_manager.deposit = token_manager.deposit.checked_sub(withdraw_amount)?;

    bank_store(deps.storage).save(key, &token_manager)?;

    send_tokens(
        deps,
        &config.xprism_token.unwrap(),
        &sender_address_raw,
        withdraw_amount,
        "withdraw_voting_tokens",
    )
}

// returns the largest locked amount in participated polls.
fn compute_locked_balance(
    storage: &mut dyn Storage,
    token_manager: &mut VotingTokenManager,
    voter: &CanonicalAddr,
) -> StdResult<Uint128> {
    // filter out not in-progress polls and get max locked
    let mut lock_entries_to_remove: Vec<u64> = vec![];
    let max_locked = token_manager
        .locked_balance
        .iter()
        .filter(|(poll_id, _)| {
            let poll: Poll = poll_read(storage).load(&poll_id.to_be_bytes()).unwrap();

            // cleanup not needed information
            if poll.status != PollStatus::InProgress {
                poll_voter_store(storage, *poll_id).remove(voter.as_slice());
                lock_entries_to_remove.push(*poll_id);
            }

            poll.status == PollStatus::InProgress
        })
        .map(|(_, v)| v.balance.u128())
        .max()
        .unwrap_or_default();

    // cleanup, check if there was any voter info removed
    token_manager
        .locked_balance
        .retain(|(poll_id, _)| !lock_entries_to_remove.contains(poll_id));

    Ok(Uint128::from(max_locked))
}

fn send_tokens(
    deps: DepsMut,
    asset_token: &CanonicalAddr,
    recipient: &CanonicalAddr,
    amount: Uint128,
    action: &str,
) -> StdResult<Response> {
    let contract_human = deps.api.addr_humanize(asset_token)?.to_string();
    let recipient_human = deps.api.addr_humanize(recipient)?.to_string();
    let attributes = vec![
        attr("action", action),
        attr("recipient", recipient_human.as_str()),
        attr("amount", &amount.to_string()),
    ];

    let r = Response::new()
        .add_attributes(attributes)
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_human,
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient_human,
                amount,
            })?,
            funds: vec![],
        }));
    Ok(r)
}

pub fn query_voting_tokens(deps: Deps, address: String) -> StdResult<VotingTokensResponse> {
    let addr_raw = deps.api.addr_canonicalize(&address).unwrap();
    let key = &addr_raw.as_slice();

    let mut token_manager = bank_read(deps.storage)
        .load(key)
        .map_err(|_| StdError::generic_err("no voting information found for this address"))?;

    // filter out not in-progress polls
    token_manager.locked_balance.retain(|(poll_id, _)| {
        let poll: Poll = poll_read(deps.storage)
            .load(&poll_id.to_be_bytes())
            .unwrap();

        poll.status == PollStatus::InProgress
    });

    Ok(VotingTokensResponse {
        balance: token_manager.deposit,
        locked_balance: token_manager.locked_balance,
    })
}
