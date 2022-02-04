#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use prism_common::parse_reply_instantiate_data;

use crate::polls::{cast_vote, create_poll, end_poll, execute_poll, failed_poll, snapshot_poll};
use crate::state::{
    config_read, config_store, poll_read, poll_voter_read, read_poll_voters, read_polls,
    read_tmp_poll_id, store_last_poll_id, Config,
};
use crate::voting::{query_voting_tokens, stake_voting_tokens, withdraw_voting_tokens};

use cosmwasm_std::{
    from_binary, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, ReplyOn,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20ReceiveMsg, MinterResponse};
use prismswap::token::InstantiateMsg as TokenInstantiateMsg;

use crate::xprism::{
    claim_redeemed_prism, mint_xprism, query_prism_withdraw_orders, query_xprism_state,
    redeem_xprism, TOTAL_PENDING_WITHDRAW,
};
use prism_protocol::common::OrderBy;
use prism_protocol::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PollExecuteMsg, PollResponse,
    PollStatus, PollsResponse, QueryMsg, VoterInfo, VotersResponse, VotersResponseItem,
};

pub const POLL_EXECUTE_REPLY_ID: u64 = 1;
pub const INSTANTIATE_REPLY_ID: u64 = 2;
pub const MIN_POLL_GAS_LIMIT: u64 = 1_000_000;

const CONTRACT_NAME: &str = "prism-gov";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    validate_quorum(msg.quorum)?;
    validate_threshold(msg.threshold)?;
    validate_poll_gas_limit(msg.poll_gas_limit)?;

    let config = Config {
        prism_token: deps.api.addr_canonicalize(&msg.prism_token)?,
        xprism_token: None,
        owner: deps.api.addr_canonicalize(info.sender.as_str())?,
        quorum: msg.quorum,
        threshold: msg.threshold,
        voting_period: msg.voting_period,
        effective_delay: msg.effective_delay,
        proposal_deposit: msg.proposal_deposit,
        snapshot_period: msg.snapshot_period,
        redemption_time: msg.redemption_time,
        poll_gas_limit: msg.poll_gas_limit,
    };

    config_store(deps.storage).save(&config)?;
    store_last_poll_id(deps.storage, 1u64)?;

    TOTAL_PENDING_WITHDRAW.save(deps.storage, &(Uint128::zero(), Uint128::zero()))?;

    Ok(Response::new().add_submessage(SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: msg.token_code_id,
            msg: to_binary(&TokenInstantiateMsg {
                name: "Prism Governace Token".to_string(),
                symbol: "xPRISM".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None,
                }),
            })?,
            funds: vec![],
            admin: None,
            label: "".to_string(),
        }
        .into(),
        id: INSTANTIATE_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            quorum,
            threshold,
            voting_period,
            effective_delay,
            proposal_deposit,
            snapshot_period,
            redemption_time,
            poll_gas_limit,
        } => update_config(
            deps,
            info,
            owner,
            quorum,
            threshold,
            voting_period,
            effective_delay,
            proposal_deposit,
            snapshot_period,
            redemption_time,
            poll_gas_limit,
        ),
        ExecuteMsg::WithdrawVotingTokens { amount } => withdraw_voting_tokens(deps, info, amount),
        ExecuteMsg::CastVote {
            poll_id,
            vote,
            amount,
        } => cast_vote(deps, env, info, poll_id, vote, amount),
        ExecuteMsg::EndPoll { poll_id } => end_poll(deps, env, poll_id),
        ExecuteMsg::ExecutePoll { poll_id } => execute_poll(deps, env, poll_id),
        ExecuteMsg::SnapshotPoll { poll_id } => snapshot_poll(deps, env, poll_id),
        ExecuteMsg::ClaimRedeemedXprism {} => claim_redeemed_prism(deps, env, info),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    // only asset contract can execute this message
    let config: Config = config_read(deps.storage).load()?;
    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    if config.xprism_token.unwrap() == sender_raw {
        match from_binary(&cw20_msg.msg) {
            Ok(Cw20HookMsg::StakeVotingTokens {}) => {
                stake_voting_tokens(deps, cw20_msg.sender, cw20_msg.amount)
            }
            Ok(Cw20HookMsg::CreatePoll {
                title,
                description,
                link,
                execute_msg,
            }) => create_poll(
                deps,
                env,
                cw20_msg.sender,
                cw20_msg.amount,
                title,
                description,
                link,
                execute_msg,
            ),
            Ok(Cw20HookMsg::RedeemXprism {}) => {
                redeem_xprism(deps, env, cw20_msg.sender, cw20_msg.amount)
            }
            _ => Err(StdError::generic_err("invalid cw20 hook message")),
        }
    } else if config.prism_token == sender_raw {
        match from_binary(&cw20_msg.msg) {
            Ok(Cw20HookMsg::MintXprism {}) => {
                mint_xprism(deps, env, cw20_msg.sender, cw20_msg.amount)
            }
            _ => Err(StdError::generic_err("invalid cw20 hook message")),
        }
    } else {
        Err(StdError::generic_err("unauthorized"))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        POLL_EXECUTE_REPLY_ID => {
            let poll_id: u64 = read_tmp_poll_id(deps.storage)?;
            failed_poll(deps, poll_id)
        }
        INSTANTIATE_REPLY_ID => set_xprism_token(deps, msg),
        _ => Err(StdError::generic_err("reply id is invalid")),
    }
}

pub fn set_xprism_token(deps: DepsMut, msg: Reply) -> StdResult<Response> {
    let mut config: Config = config_read(deps.storage).load()?;

    if config.xprism_token.is_some() {
        // should never happen
        return Err(StdError::generic_err("xprism token was already set"));
    }

    let res = parse_reply_instantiate_data(msg)
        .map_err(|_| StdError::generic_err("error parsing xprism instantiation reply"))?;
    let xprism_token_addr = deps.api.addr_validate(&res.contract_address)?;

    config.xprism_token = Some(deps.api.addr_canonicalize(xprism_token_addr.as_str())?);

    config_store(deps.storage).save(&config)?;

    Ok(Response::new().add_attribute("xprism_token_addr", xprism_token_addr))
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    quorum: Option<Decimal>,
    threshold: Option<Decimal>,
    voting_period: Option<u64>,
    effective_delay: Option<u64>,
    proposal_deposit: Option<Uint128>,
    snapshot_period: Option<u64>,
    redemption_time: Option<u64>,
    poll_gas_limit: Option<u64>,
) -> StdResult<Response> {
    let api = deps.api;
    config_store(deps.storage).update(|mut config| {
        if config.owner != api.addr_canonicalize(info.sender.as_str())? {
            return Err(StdError::generic_err("unauthorized"));
        }

        if let Some(owner) = owner {
            config.owner = api.addr_canonicalize(&owner)?;
        }

        if let Some(quorum) = quorum {
            validate_quorum(quorum)?;
            config.quorum = quorum;
        }

        if let Some(threshold) = threshold {
            validate_threshold(threshold)?;
            config.threshold = threshold;
        }

        if let Some(voting_period) = voting_period {
            config.voting_period = voting_period;
        }

        if let Some(effective_delay) = effective_delay {
            config.effective_delay = effective_delay;
        }

        if let Some(proposal_deposit) = proposal_deposit {
            config.proposal_deposit = proposal_deposit;
        }

        if let Some(snapshot_period) = snapshot_period {
            config.snapshot_period = snapshot_period;
        }

        if let Some(redemption_time) = redemption_time {
            config.redemption_time = redemption_time;
        }

        if let Some(poll_gas_limit) = poll_gas_limit {
            validate_poll_gas_limit(poll_gas_limit)?;
            config.poll_gas_limit = poll_gas_limit;
        }

        Ok(config)
    })?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::VotingTokens { address } => to_binary(&query_voting_tokens(deps, address)?),
        QueryMsg::Poll { poll_id } => to_binary(&query_poll(deps, poll_id)?),
        QueryMsg::Polls {
            filter,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_polls(deps, filter, start_after, limit, order_by)?),
        QueryMsg::Voter { poll_id, address } => to_binary(&query_voter(deps, poll_id, address)?),
        QueryMsg::Voters {
            poll_id,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_voters(deps, poll_id, start_after, limit, order_by)?),
        QueryMsg::PrismWithdrawOrders {
            address,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_prism_withdraw_orders(
            deps,
            env,
            address,
            start_after,
            limit,
            order_by,
        )?),
        QueryMsg::XprismState {} => to_binary(&query_xprism_state(deps, env)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = config_read(deps.storage).load()?;
    Ok(ConfigResponse {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        prism_token: deps.api.addr_humanize(&config.prism_token)?.to_string(),
        xprism_token: deps
            .api
            .addr_humanize(&config.xprism_token.unwrap())?
            .to_string(),
        quorum: config.quorum,
        threshold: config.threshold,
        voting_period: config.voting_period,
        effective_delay: config.effective_delay,
        proposal_deposit: config.proposal_deposit,
        snapshot_period: config.snapshot_period,
        redemption_time: config.redemption_time,
        poll_gas_limit: config.poll_gas_limit,
    })
}

fn query_poll(deps: Deps, poll_id: u64) -> StdResult<PollResponse> {
    let poll = match poll_read(deps.storage).may_load(&poll_id.to_be_bytes())? {
        Some(poll) => poll,
        None => return Err(StdError::generic_err("Poll does not exist")),
    };

    Ok(PollResponse {
        id: poll.id,
        creator: deps.api.addr_humanize(&poll.creator).unwrap().to_string(),
        status: poll.status,
        end_time: poll.end_time,
        title: poll.title,
        description: poll.description,
        link: poll.link,
        deposit_amount: poll.deposit_amount,
        execute_data: if let Some(execute_data) = poll.execute_data {
            Some(PollExecuteMsg {
                contract: deps.api.addr_humanize(&execute_data.contract)?.to_string(),
                msg: execute_data.msg,
            })
        } else {
            None
        },
        yes_votes: poll.yes_votes,
        no_votes: poll.no_votes,
        abstain_votes: poll.abstain_votes,
        supply_snapshot: poll.supply_snapshot,
        required_quorum: poll.required_quorum,
        required_threshold: poll.required_threshold,
    })
}

fn query_polls(
    deps: Deps,
    filter: Option<PollStatus>,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PollsResponse> {
    let polls = read_polls(deps.storage, filter, start_after, limit, order_by)?;
    let poll_responses: StdResult<Vec<PollResponse>> = polls
        .iter()
        .map(|poll| {
            Ok(PollResponse {
                id: poll.id,
                creator: deps.api.addr_humanize(&poll.creator).unwrap().to_string(),
                status: poll.status.clone(),
                end_time: poll.end_time,
                title: poll.title.to_string(),
                description: poll.description.to_string(),
                link: poll.link.clone(),
                deposit_amount: poll.deposit_amount,
                execute_data: if let Some(execute_data) = poll.execute_data.clone() {
                    Some(PollExecuteMsg {
                        contract: deps.api.addr_humanize(&execute_data.contract)?.to_string(),
                        msg: execute_data.msg,
                    })
                } else {
                    None
                },
                yes_votes: poll.yes_votes,
                no_votes: poll.no_votes,
                abstain_votes: poll.abstain_votes,
                supply_snapshot: poll.supply_snapshot,
                required_quorum: poll.required_quorum,
                required_threshold: poll.required_threshold,
            })
        })
        .collect();

    Ok(PollsResponse {
        polls: poll_responses?,
    })
}

fn query_voter(deps: Deps, poll_id: u64, address: String) -> StdResult<VotersResponseItem> {
    let voter: VoterInfo = poll_voter_read(deps.storage, poll_id)
        .load(deps.api.addr_canonicalize(&address)?.as_slice())?;
    Ok(VotersResponseItem {
        voter: address,
        vote: voter.vote,
        balance: voter.balance,
    })
}

fn query_voters(
    deps: Deps,
    poll_id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<VotersResponse> {
    let voters = if let Some(start_after) = start_after {
        read_poll_voters(
            deps.storage,
            poll_id,
            Some(deps.api.addr_canonicalize(&start_after)?),
            limit,
            order_by,
        )?
    } else {
        read_poll_voters(deps.storage, poll_id, None, limit, order_by)?
    };

    let voters_response: StdResult<Vec<VotersResponseItem>> = voters
        .iter()
        .map(|voter_info| {
            Ok(VotersResponseItem {
                voter: deps.api.addr_humanize(&voter_info.0)?.to_string(),
                vote: voter_info.1.vote.clone(),
                balance: voter_info.1.balance,
            })
        })
        .collect();

    Ok(VotersResponse {
        voters: voters_response?,
    })
}

/// validate_quorum returns an error if the quorum is invalid
/// (we require 0-1)
fn validate_quorum(quorum: Decimal) -> StdResult<()> {
    if quorum > Decimal::one() {
        Err(StdError::generic_err("quorum must be 0 to 1"))
    } else {
        Ok(())
    }
}

/// validate_threshold returns an error if the threshold is invalid
/// (we require 0-1)
fn validate_threshold(threshold: Decimal) -> StdResult<()> {
    if threshold > Decimal::one() {
        Err(StdError::generic_err("threshold must be 0 to 1"))
    } else {
        Ok(())
    }
}

/// validate_threshold returns an error if the threshold is invalid
/// (we require 0-1)
fn validate_poll_gas_limit(poll_gas_limit: u64) -> StdResult<()> {
    if poll_gas_limit < MIN_POLL_GAS_LIMIT {
        Err(StdError::generic_err(format!(
            "gas limit can not be smaller than {}",
            MIN_POLL_GAS_LIMIT
        )))
    } else {
        Ok(())
    }
}
