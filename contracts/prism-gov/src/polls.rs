use crate::contract::POLL_EXECUTE_REPLY_ID;
use crate::state::{
    bank_read, bank_store, config_read, config_store, poll_indexer_store, poll_store,
    poll_voter_read, poll_voter_store, pop_last_poll_id, store_tmp_poll_id, Config, ExecuteData,
    Poll,
};

use astroport::querier::query_supply;
use cosmwasm_std::{
    attr, to_binary, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, ReplyOn, Response, StdError,
    StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::gov::{PollExecuteMsg, PollStatus, VoteOption, VoterInfo};

const MIN_TITLE_LENGTH: usize = 4;
const MAX_TITLE_LENGTH: usize = 64;
const MIN_DESC_LENGTH: usize = 4;
const MAX_DESC_LENGTH: usize = 256;
const MIN_LINK_LENGTH: usize = 12;
const MAX_LINK_LENGTH: usize = 128;

/*
 * Creates a new poll
 */
#[allow(clippy::too_many_arguments)]
pub fn create_poll(
    deps: DepsMut,
    env: Env,
    proposer: String,
    deposit_amount: Uint128,
    title: String,
    description: String,
    link: Option<String>,
    poll_execute_msg: Option<PollExecuteMsg>,
) -> StdResult<Response> {
    validate_title(&title)?;
    validate_description(&description)?;
    validate_link(&link)?;

    let config: Config = config_store(deps.storage).load()?;
    if deposit_amount < config.proposal_deposit {
        return Err(StdError::generic_err(format!(
            "Must deposit more than {} token",
            config.proposal_deposit
        )));
    }

    let poll_execute_data = if let Some(poll_execute_msg) = poll_execute_msg {
        Some(ExecuteData {
            contract: deps.api.addr_canonicalize(&poll_execute_msg.contract)?,
            msg: poll_execute_msg.msg,
        })
    } else {
        None
    };

    let sender_address_raw = deps.api.addr_canonicalize(&proposer)?;
    let current_seconds = env.block.time.seconds();
    let poll_id = pop_last_poll_id(deps.storage)?;
    let new_poll = Poll {
        id: poll_id,
        creator: sender_address_raw,
        status: PollStatus::InProgress,
        yes_votes: Uint128::zero(),
        no_votes: Uint128::zero(),
        abstain_votes: Uint128::zero(),
        end_time: current_seconds + config.voting_period,
        title,
        description,
        link,
        execute_data: poll_execute_data,
        deposit_amount,
        supply_snapshot: None,
    };

    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &new_poll)?;
    poll_indexer_store(deps.storage, &PollStatus::InProgress)
        .save(&poll_id.to_be_bytes(), &true)?;

    let r = Response::new().add_attributes(vec![
        attr("action", "create_poll"),
        attr(
            "creator",
            deps.api.addr_humanize(&new_poll.creator)?.as_str(),
        ),
        attr("poll_id", &poll_id.to_string()),
        attr("end_time", new_poll.end_time.to_string()),
    ]);
    Ok(r)
}

/*
 * Ends a poll.
 */
pub fn end_poll(deps: DepsMut, env: Env, poll_id: u64) -> StdResult<Response> {
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    let current_seconds = env.block.time.seconds();
    if a_poll.end_time > current_seconds {
        return Err(StdError::generic_err("Voting period has not expired"));
    }

    let no = a_poll.no_votes.u128();
    let yes = a_poll.yes_votes.u128();
    let abstain = a_poll.abstain_votes.u128();

    let tallied_weight = yes + no + abstain;

    let mut poll_status = PollStatus::Rejected;
    let mut rejected_reason = "";
    let mut passed = false;

    let mut messages: Vec<CosmosMsg> = vec![];
    let config: Config = config_read(deps.storage).load()?;

    let total_supply = match a_poll.supply_snapshot {
        Some(v) => v,
        None => {
            let supply =
                query_supply(&deps.querier, deps.api.addr_humanize(&config.xprism_token)?)?;
            a_poll.supply_snapshot = Some(supply);

            supply
        }
    };

    let quorum = if total_supply.is_zero() {
        Decimal::zero()
    } else {
        Decimal::from_ratio(tallied_weight, total_supply)
    };

    if tallied_weight == 0 || quorum < config.quorum {
        // Quorum: More than quorum of the total staked tokens at the end of the voting
        // period need to have participated in the vote.
        rejected_reason = "Quorum not reached";
    } else {
        if yes != 0u128 && Decimal::from_ratio(yes, yes + no) > config.threshold {
            //Threshold: More than 50% of the tokens that participated in the vote
            // (after excluding “Abstain” votes) need to have voted in favor of the proposal (“Yes”).
            poll_status = PollStatus::Passed;
            passed = true;
        } else {
            rejected_reason = "Threshold not reached";
        }

        // Refunds deposit only when quorum is reached
        if !a_poll.deposit_amount.is_zero() {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.xprism_token)?.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: deps.api.addr_humanize(&a_poll.creator)?.to_string(),
                    amount: a_poll.deposit_amount,
                })?,
            }))
        }
    }

    // Update poll indexer
    poll_indexer_store(deps.storage, &PollStatus::InProgress).remove(&a_poll.id.to_be_bytes());
    poll_indexer_store(deps.storage, &poll_status).save(&a_poll.id.to_be_bytes(), &true)?;

    // Update poll status
    a_poll.status = poll_status;
    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "end_poll"),
        attr("poll_id", &poll_id.to_string()),
        attr("rejected_reason", rejected_reason),
        attr("passed", &passed.to_string()),
    ]))
}

/*
 * Execute a msg of passed poll.
 */
pub fn execute_poll(deps: DepsMut, env: Env, poll_id: u64) -> StdResult<Response> {
    let config: Config = config_read(deps.storage).load()?;
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::Passed {
        return Err(StdError::generic_err("Poll is not in passed status"));
    }

    let current_seconds = env.block.time.seconds();
    if a_poll.end_time + config.effective_delay > current_seconds {
        return Err(StdError::generic_err("Effective delay has not expired"));
    }

    poll_indexer_store(deps.storage, &PollStatus::Passed).remove(&poll_id.to_be_bytes());
    poll_indexer_store(deps.storage, &PollStatus::Executed).save(&poll_id.to_be_bytes(), &true)?;

    a_poll.status = PollStatus::Executed;
    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    let mut messages: Vec<SubMsg> = vec![];
    if let Some(execute_data) = a_poll.execute_data {
        messages.push(SubMsg {
            msg: CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&execute_data.contract)?.to_string(),
                msg: execute_data.msg,
                funds: vec![],
            }),
            gas_limit: None,
            id: POLL_EXECUTE_REPLY_ID,
            reply_on: ReplyOn::Error,
        });
        store_tmp_poll_id(deps.storage, a_poll.id)?;
    } else {
        return Err(StdError::generic_err("The poll does not have execute_data"));
    }

    Ok(Response::new()
        .add_submessages(messages)
        .add_attributes(vec![
            attr("action", "execute_poll"),
            attr("poll_id", poll_id.to_string()),
        ]))
}

/*
 * If the executed message of a passed poll fails, it is marked as failed
 */
pub fn failed_poll(deps: DepsMut, poll_id: u64) -> StdResult<Response> {
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    poll_indexer_store(deps.storage, &PollStatus::Executed).remove(&poll_id.to_be_bytes());
    poll_indexer_store(deps.storage, &PollStatus::Failed).save(&poll_id.to_be_bytes(), &true)?;

    a_poll.status = PollStatus::Failed;
    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_attribute("action", "failed_poll"))
}

/*
 * User casts a vote on the provided poll id
 */
pub fn cast_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    poll_id: u64,
    vote: VoteOption,
    amount: Uint128,
) -> StdResult<Response> {
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let config = config_read(deps.storage).load()?;

    let mut a_poll: Poll = poll_store(deps.storage)
        .load(&poll_id.to_be_bytes())
        .map_err(|_| StdError::generic_err("Poll does not exist"))?;
    let current_seconds = env.block.time.seconds();
    if a_poll.status != PollStatus::InProgress || current_seconds > a_poll.end_time {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    // Check the voter already has a vote on the poll
    if poll_voter_read(deps.storage, poll_id)
        .load(sender_address_raw.as_slice())
        .is_ok()
    {
        return Err(StdError::generic_err("User has already voted."));
    }

    let key = &sender_address_raw.as_slice();
    let mut token_manager = bank_read(deps.storage).may_load(key)?.unwrap_or_default();

    if token_manager.deposit < amount {
        return Err(StdError::generic_err(
            "User does not have enough staked tokens.",
        ));
    }

    // update tally info
    match vote {
        VoteOption::Yes => a_poll.yes_votes += amount,
        VoteOption::No => a_poll.no_votes += amount,
        VoteOption::Abstain => a_poll.abstain_votes += amount,
    }

    let vote_info = VoterInfo {
        vote,
        balance: amount,
    };
    token_manager
        .locked_balance
        .push((poll_id, vote_info.clone()));
    bank_store(deps.storage).save(key, &token_manager)?;

    // store poll voter && and update poll data
    poll_voter_store(deps.storage, poll_id).save(sender_address_raw.as_slice(), &vote_info)?;

    // processing snapshot
    let time_to_end = a_poll.end_time - current_seconds;
    if time_to_end < config.snapshot_period && a_poll.supply_snapshot.is_none() {
        let supply = query_supply(&deps.querier, deps.api.addr_humanize(&config.xprism_token)?)?;
        a_poll.supply_snapshot = Some(supply);
    }

    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "cast_vote"),
        attr("poll_id", &poll_id.to_string()),
        attr("amount", &amount.to_string()),
        attr("voter", &info.sender.to_string()),
        attr("vote_option", vote_info.vote.to_string()),
    ]))
}

/*
 * SnapshotPoll is used to take a snapshot of the token supply for quorum calculation
 */
pub fn snapshot_poll(deps: DepsMut, env: Env, poll_id: u64) -> StdResult<Response> {
    let config: Config = config_read(deps.storage).load()?;
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    let current_seconds = env.block.time.seconds();
    let time_to_end = a_poll.end_time - current_seconds;

    if time_to_end > config.snapshot_period {
        return Err(StdError::generic_err("Cannot snapshot at this height"));
    }

    if a_poll.supply_snapshot.is_some() {
        return Err(StdError::generic_err("Snapshot has already occurred"));
    }

    // store the current supply amount for quorum calculation
    let supply = query_supply(&deps.querier, deps.api.addr_humanize(&config.xprism_token)?)?;

    a_poll.supply_snapshot = Some(supply);

    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "snapshot_poll"),
        attr("poll_id", poll_id.to_string()),
        attr("supply_snapshot", supply),
    ]))
}

/// validate_title returns an error if the title is invalid
fn validate_title(title: &str) -> StdResult<()> {
    if title.len() < MIN_TITLE_LENGTH {
        Err(StdError::generic_err("Title too short"))
    } else if title.len() > MAX_TITLE_LENGTH {
        Err(StdError::generic_err("Title too long"))
    } else {
        Ok(())
    }
}

/// validate_description returns an error if the description is invalid
fn validate_description(description: &str) -> StdResult<()> {
    if description.len() < MIN_DESC_LENGTH {
        Err(StdError::generic_err("Description too short"))
    } else if description.len() > MAX_DESC_LENGTH {
        Err(StdError::generic_err("Description too long"))
    } else {
        Ok(())
    }
}

/// validate_link returns an error if the link is invalid
fn validate_link(link: &Option<String>) -> StdResult<()> {
    if let Some(link) = link {
        if link.len() < MIN_LINK_LENGTH {
            Err(StdError::generic_err("Link too short"))
        } else if link.len() > MAX_LINK_LENGTH {
            Err(StdError::generic_err("Link too long"))
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}
