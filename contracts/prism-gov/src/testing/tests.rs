use crate::contract::{execute, instantiate, query, reply, MIN_POLL_GAS_LIMIT};
use crate::polls::MAX_POLL_VOTES_PER_USER;
use crate::state::{
    bank_read, bank_store, config_read, poll_store, poll_voter_read, poll_voter_store, Config,
    Poll, VotingTokenManager,
};
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, coins, from_binary, to_binary, Addr, Api, CanonicalAddr, ContractResult, CosmosMsg,
    Decimal, DepsMut, Env, Reply, ReplyOn, Response, StdError, SubMsg, SubMsgExecutionResponse,
    Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use prism_common::testing::mock_querier::mock_dependencies;
use prism_protocol::common::OrderBy;
use prism_protocol::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PollExecuteMsg, PollResponse,
    PollStatus, PollsResponse, PrismWithdrawOrdersResponse, QueryMsg, VoteOption, VoterInfo,
    VotersResponse, VotersResponseItem, VotingTokensResponse, XprismStateResponse,
};
use prismswap::token::InstantiateMsg as TokenInstantiateMsg;

const VOTING_TOKEN: &str = "xprism0000";
const PRISM_TOKEN: &str = "prism_token";
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";
const TEST_VOTER_2: &str = "voter2";
const TEST_VOTER_3: &str = "voter3";
const DEFAULT_QUORUM: u64 = 30u64;
const DEFAULT_THRESHOLD: u64 = 50u64;
const DEFAULT_VOTING_PERIOD: u64 = 10000u64;
const DEFAULT_EFFECTIVE_DELAY: u64 = 10000u64;
const DEFAULT_PROPOSAL_DEPOSIT: u128 = 10000000000u128;
const DEFAULT_SNAPSHOT_PERIOD: u64 = 10u64;
const DEFAULT_REDEMPTION_TIME: u64 = 21u64 * 24u64 * 60u64 * 60u64;
const DEFAULT_POLL_GAS_LIMIT: u64 = 1000000;
const DEFAULT_TOKEN_CODE_ID: u64 = 1;

fn mock_instantiate(deps: DepsMut) {
    let msg = InstantiateMsg {
        prism_token: PRISM_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
        redemption_time: DEFAULT_REDEMPTION_TIME,
        poll_gas_limit: DEFAULT_POLL_GAS_LIMIT,
        token_code_id: DEFAULT_TOKEN_CODE_ID,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps, mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");
}

fn mock_reply(deps: DepsMut) {
    let reply_msg = Reply {
        id: 2,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(vec![10, 10, 120, 112, 114, 105, 115, 109, 48, 48, 48, 48].into()),
        }),
    };
    let _res = reply(deps, mock_env(), reply_msg).unwrap();
}

fn mock_env_height(height: u64, time: u64) -> Env {
    let mut env = mock_env();
    env.block.height = height;
    env.block.time = Timestamp::from_seconds(time);
    env
}

fn init_msg() -> InstantiateMsg {
    InstantiateMsg {
        prism_token: PRISM_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
        redemption_time: DEFAULT_REDEMPTION_TIME,
        poll_gas_limit: DEFAULT_POLL_GAS_LIMIT,
        token_code_id: DEFAULT_TOKEN_CODE_ID,
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let mut msg = init_msg();
    msg.poll_gas_limit = MIN_POLL_GAS_LIMIT - 1;
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err(format!(
            "gas limit can not be smaller than {}",
            MIN_POLL_GAS_LIMIT
        ))
    );

    let msg = init_msg();
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(1, res.messages.len());

    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: DEFAULT_TOKEN_CODE_ID,
                msg: to_binary(&TokenInstantiateMsg {
                    name: "Prism Governance Token".to_string(),
                    symbol: "xPRISM".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: MOCK_CONTRACT_ADDR.to_string(),
                        cap: None,
                    }),
                })
                .unwrap(),
                funds: vec![],
                admin: None,
                label: "".to_string(),
            }
            .into(),
            id: 2,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        }]
    );

    let reply_msg = Reply {
        id: 2,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(vec![10, 10, 120, 112, 114, 105, 115, 109, 48, 48, 48, 48].into()),
        }),
    };
    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![attr("xprism_token_addr", "xprism0000")]
    );

    let config: Config = config_read(&deps.storage).load().unwrap();
    assert_eq!(
        config,
        Config {
            owner: deps.api.addr_canonicalize(TEST_CREATOR).unwrap(),
            xprism_token: Some(deps.api.addr_canonicalize(VOTING_TOKEN).unwrap()),
            prism_token: deps.api.addr_canonicalize(PRISM_TOKEN).unwrap(),
            quorum: Decimal::percent(DEFAULT_QUORUM),
            threshold: Decimal::percent(DEFAULT_THRESHOLD),
            voting_period: DEFAULT_VOTING_PERIOD,
            effective_delay: DEFAULT_EFFECTIVE_DELAY,
            proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
            redemption_time: DEFAULT_REDEMPTION_TIME,
            poll_gas_limit: DEFAULT_POLL_GAS_LIMIT,
        }
    );
}

#[test]
fn poll_not_found() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 });

    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
        Err(e) => panic!("Unexpected error: {:?}", e),
        _ => panic!("Must return error"),
    }
}

#[test]
fn fails_create_poll_invalid_quorum() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        quorum: Decimal::percent(101),
        ..init_msg()
    };

    let res = instantiate(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "quorum must be 0 to 1"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_create_poll_invalid_threshold() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        threshold: Decimal::percent(101),
        ..init_msg()
    };

    let res = instantiate(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "threshold must be 0 to 1"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_create_poll_invalid_title() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = create_poll_msg("a".to_string(), "test".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Title too short"),
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string(),
            "test".to_string(),
            None,
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Title too long"),
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_description() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "a".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Description too short"),
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "test".to_string(),
            "0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string(),
            None,
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Description too long"),
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_link() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("http://hih".to_string()),
        None,
    );
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Link too short"),
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "test".to_string(),
            "test".to_string(),
            Some("0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string()),
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Link too long"),
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_deposit() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_CREATOR.to_string(),
        amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT - 1),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title: "TESTTEST".to_string(),
            description: "TESTTEST".to_string(),
            link: None,
            execute_msg: None,
        })
        .unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            format!("Must deposit more than {} token", DEFAULT_PROPOSAL_DEPOSIT)
        ),
        Err(_) => panic!("Unknown error"),
    }
}

fn create_poll_msg(
    title: String,
    description: String,
    link: Option<String>,
    execute_msg: Option<PollExecuteMsg>,
) -> ExecuteMsg {
    ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_CREATOR.to_string(),
        amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title,
            description,
            link,
            execute_msg,
        })
        .unwrap(),
    })
}

#[test]
fn happy_days_create_poll() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).seconds(),
        TEST_CREATOR,
        execute_res,
    );
}

#[test]
fn query_polls() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("http://google.com".to_string()),
        None,
    );
    let _execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let msg = create_poll_msg("test2".to_string(), "test2".to_string(), None, None);
    let _execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![
            PollResponse {
                id: 1u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                end_time: 10000u64,
                title: "test".to_string(),
                description: "test".to_string(),
                link: Some("http://google.com".to_string()),
                deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
                execute_data: None,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                supply_snapshot: None,
                required_quorum: Decimal::percent(DEFAULT_QUORUM),
                required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
            },
            PollResponse {
                id: 2u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                end_time: 10000u64,
                title: "test2".to_string(),
                description: "test2".to_string(),
                link: None,
                deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
                execute_data: None,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                supply_snapshot: None,
                required_quorum: Decimal::percent(DEFAULT_QUORUM),
                required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
            },
        ]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: Some(1u64),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 2u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_time: 10000u64,
            title: "test2".to_string(),
            description: "test2".to_string(),
            link: None,
            deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            abstain_votes: Uint128::zero(),
            supply_snapshot: None,
            required_quorum: Decimal::percent(DEFAULT_QUORUM),
            required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: Some(2u64),
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 1u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_time: 10000u64,
            title: "test".to_string(),
            description: "test".to_string(),
            link: Some("http://google.com".to_string()),
            deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            abstain_votes: Uint128::zero(),
            supply_snapshot: None,
            required_quorum: Decimal::percent(DEFAULT_QUORUM),
            required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
        }]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: Some(1u64),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 2u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_time: 10000u64,
            title: "test2".to_string(),
            description: "test2".to_string(),
            link: None,
            deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            abstain_votes: Uint128::zero(),
            supply_snapshot: None,
            required_quorum: Decimal::percent(DEFAULT_QUORUM),
            required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls, vec![]);
}

#[test]
fn create_poll_no_quorum() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(1, DEFAULT_VOTING_PERIOD, TEST_CREATOR, execute_res);
}

#[test]
fn fails_end_poll_before_end_time() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(1, DEFAULT_VOTING_PERIOD, TEST_CREATOR, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(DEFAULT_VOTING_PERIOD, value.end_time);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let env = mock_env_height(0, 0);
    let info = mock_info(TEST_CREATOR, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg);

    match execute_res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Voting period has not expired"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_end_poll() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 10000000000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();
    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .seconds(),
        TEST_CREATOR,
        execute_res,
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(stake_amount, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", stake_amount.to_string()),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    // not in passed status
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap_err();
    match execute_res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Poll is not in passed status"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // End poll will withdraw deposit balance
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(stake_amount as u128),
        )],
    )]);

    // effective delay has not expired
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap_err();
    match execute_res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Effective delay has not expired"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_EFFECTIVE_DELAY);
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg {
            msg: CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz,
                funds: vec![],
            }),
            gas_limit: Some(DEFAULT_POLL_GAS_LIMIT),
            id: 1u64,
            reply_on: ReplyOn::Error,
        }]
    );
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1"),]
    );

    // Query executed polls
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Executed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 1);

    // staker locked token should have disappeared
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::VotingTokens {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: VotingTokensResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        VotingTokensResponse {
            balance: Uint128::new(stake_amount),
            locked_balance: vec![],
        }
    );
}

#[test]
fn failed_execute_poll() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 10000000000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();
    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .seconds(),
        TEST_CREATOR,
        execute_res,
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(stake_amount, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", stake_amount.to_string()),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // Try to execute the poll
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_EFFECTIVE_DELAY);
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg {
            msg: CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: exec_msg_bz,
                funds: vec![],
            }),
            gas_limit: Some(DEFAULT_POLL_GAS_LIMIT),
            id: 1u64,
            reply_on: ReplyOn::Error,
        }]
    );
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1")]
    );

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Err("Error".to_string()),
    };
    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "failed_poll")]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let poll_res: PollResponse = from_binary(&res).unwrap();
    assert_eq!(poll_res.status, PollStatus::Failed);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Failed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let polls_res: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(polls_res.polls[0], poll_res);
}

#[test]
fn end_poll_zero_quorum() {
    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let mut creator_env = mock_env_height(1000, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: Uint128::new(123),
            })
            .unwrap(),
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .seconds(),
        TEST_CREATOR,
        execute_res,
    );
    let stake_amount = 100;
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(100u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // Query rejected polls
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Rejected),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 1);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);
}

#[test]
fn end_poll_quorum_rejected() {
    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .seconds();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    let stake_amount = 100;
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(100u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(stake_amount, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", "1"),
            attr("amount", "10"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn end_poll_quorum_rejected_noting_staked() {
    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .seconds();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    let env = mock_env_height(0, end_time);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(1000000u128))],
    )]);

    let execute_res = execute(deps.as_mut(), env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_nay_rejected() {
    let voter1_stake = 10000000000;
    let voter2_stake = 100000000000;
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .seconds();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((voter1_stake + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(voter1_stake as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(voter1_stake, execute_res);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((voter1_stake + voter2_stake + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(voter2_stake as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(voter2_stake, execute_res);

    let info = mock_info(TEST_VOTER_2, &[]);
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::No,
        amount: Uint128::from(voter2_stake),
    };
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, voter2_stake, 1, VoteOption::No, execute_res);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Threshold not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn fails_cast_vote_not_enough_staked() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(1, DEFAULT_VOTING_PERIOD, TEST_CREATOR, execute_res);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(10u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(10, execute_res);

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(11u128),
    };

    let res = execute(deps.as_mut(), env, info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "User does not have enough staked tokens.")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_cast_vote() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(1, DEFAULT_VOTING_PERIOD, TEST_CREATOR, execute_res);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(12u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let amount = 10u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    // Query staker
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::VotingTokens {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: VotingTokensResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        VotingTokensResponse {
            balance: Uint128::new(11u128),
            locked_balance: vec![(
                1u64,
                VoterInfo {
                    vote: VoteOption::Yes,
                    balance: Uint128::from(amount),
                }
            )],
        }
    );

    // Query voter
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voter {
            poll_id: 1u64,
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: VotersResponseItem = from_binary(&res).unwrap();
    assert_eq!(
        response,
        VotersResponseItem {
            voter: TEST_VOTER.to_string(),
            vote: VoteOption::Yes,
            balance: Uint128::from(amount),
        }
    );

    // Query voters
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.voters,
        vec![VotersResponseItem {
            voter: TEST_VOTER.to_string(),
            vote: VoteOption::Yes,
            balance: Uint128::from(amount),
        }]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: Some(TEST_VOTER.to_string()),
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(response.voters.len(), 0);

    // Add another voter
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(1u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(1, execute_res);

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER_2, &coins(1, VOTING_TOKEN));
    let amount = 1u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, amount, 1, VoteOption::Yes, execute_res);

    //Query voters ascending
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.voters,
        vec![
            VotersResponseItem {
                voter: TEST_VOTER_2.to_string(),
                vote: VoteOption::Yes,
                balance: Uint128::from(1u128),
            },
            VotersResponseItem {
                voter: TEST_VOTER.to_string(),
                vote: VoteOption::Yes,
                balance: Uint128::from(10u128),
            }
        ]
    );

    //Query voters ascending limit 1
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: None,
            limit: Some(1u32),
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.voters,
        vec![VotersResponseItem {
            voter: TEST_VOTER_2.to_string(),
            vote: VoteOption::Yes,
            balance: Uint128::from(1u128),
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: Some(TEST_VOTER_2.to_string()),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.voters,
        vec![VotersResponseItem {
            voter: TEST_VOTER.to_string(),
            vote: VoteOption::Yes,
            balance: Uint128::from(10u128),
        }]
    );
}

#[test]
fn happy_days_withdraw_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);

    // double the balance, only half will be withdrawn
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(22u128))],
    )]);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = execute_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(11u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn happy_days_withdraw_voting_tokens_all() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens { amount: None };

    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = execute_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(11u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn withdraw_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);

    // make fake polls; one in progress & one in passed
    poll_store(&mut deps.storage)
        .save(
            &1u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: CanonicalAddr::from(vec![]),
                status: PollStatus::InProgress,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                supply_snapshot: None,
                required_quorum: Decimal::percent(DEFAULT_QUORUM),
                required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
            },
        )
        .unwrap();

    poll_store(&mut deps.storage)
        .save(
            &2u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: CanonicalAddr::from(vec![]),
                status: PollStatus::Passed,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                supply_snapshot: None,
                required_quorum: Decimal::percent(DEFAULT_QUORUM),
                required_threshold: Decimal::percent(DEFAULT_THRESHOLD),
            },
        )
        .unwrap();

    let voter_addr_raw = deps.api.addr_canonicalize(TEST_VOTER).unwrap();
    poll_voter_store(&mut deps.storage, 1u64)
        .save(
            voter_addr_raw.as_slice(),
            &VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::new(5u128),
            },
        )
        .unwrap();
    poll_voter_store(&mut deps.storage, 2u64)
        .save(
            voter_addr_raw.as_slice(),
            &VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::new(5u128),
            },
        )
        .unwrap();
    bank_store(&mut deps.storage)
        .save(
            voter_addr_raw.as_slice(),
            &VotingTokenManager {
                deposit: Uint128::new(11u128),
                locked_balance: vec![
                    (
                        1u64,
                        VoterInfo {
                            vote: VoteOption::Yes,
                            balance: Uint128::new(5u128),
                        },
                    ),
                    (
                        2u64,
                        VoterInfo {
                            vote: VoteOption::Yes,
                            balance: Uint128::new(5u128),
                        },
                    ),
                ],
            },
        )
        .unwrap();

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(5u128)),
    };

    let _ = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let voter = poll_voter_read(&deps.storage, 1u64)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        voter,
        VoterInfo {
            vote: VoteOption::Yes,
            balance: Uint128::new(5u128),
        }
    );

    let token_manager = bank_read(&deps.storage)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        token_manager.locked_balance,
        vec![(
            1u64,
            VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::new(5u128),
            }
        )]
    );
}

#[test]
fn fails_withdraw_voting_tokens_no_stake() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "no voting information found for this address")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_withdraw_too_many_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(10u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(10, execute_res);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "User is trying to withdraw too many tokens")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_twice() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).seconds(),
        TEST_CREATOR,
        execute_res,
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);

    let amount = 1u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };
    let res = execute(deps.as_mut(), env, info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "User has already voted."),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_without_poll() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = ExecuteMsg::CastVote {
        poll_id: 0,
        vote: VoteOption::Yes,
        amount: Uint128::from(1u128),
    };
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_stake_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);
}

#[test]
fn fails_insufficient_funds() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // insufficient token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(0u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_staking_wrong_token() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    // wrong token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(&(VOTING_TOKEN.to_string() + "2"), &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

// helper to confirm the expected create_poll response
fn assert_create_poll_result(poll_id: u64, end_time: u64, creator: &str, execute_res: Response) {
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", creator),
            attr("poll_id", poll_id.to_string()),
            attr("end_time", end_time.to_string()),
        ]
    );
}

fn assert_stake_tokens_result(new_amount: u128, execute_res: Response) {
    assert_eq!(
        execute_res.attributes.get(2).expect("no log"),
        &attr("amount", new_amount.to_string())
    );
}

fn assert_cast_vote_success(
    voter: &str,
    amount: u128,
    poll_id: u64,
    vote_option: VoteOption,
    execute_res: Response,
) {
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", poll_id.to_string()),
            attr("amount", amount.to_string()),
            attr("voter", voter),
            attr("vote_option", vote_option.to_string()),
        ]
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // update owner
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("addr0001".to_string()),
        quorum: None,
        threshold: None,
        voting_period: None,
        effective_delay: None,
        proposal_deposit: None,
        snapshot_period: None,
        redemption_time: None,
        poll_gas_limit: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();

    assert_eq!(
        config,
        ConfigResponse {
            owner: "addr0001".to_string(),
            prism_token: PRISM_TOKEN.to_string(),
            xprism_token: VOTING_TOKEN.to_string(),
            quorum: Decimal::percent(DEFAULT_QUORUM),
            threshold: Decimal::percent(DEFAULT_THRESHOLD),
            voting_period: DEFAULT_VOTING_PERIOD,
            effective_delay: DEFAULT_EFFECTIVE_DELAY,
            proposal_deposit: Uint128::from(DEFAULT_PROPOSAL_DEPOSIT),
            snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
            redemption_time: DEFAULT_REDEMPTION_TIME,
            poll_gas_limit: DEFAULT_POLL_GAS_LIMIT
        }
    );

    // update left items
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        quorum: Some(Decimal::percent(20)),
        threshold: Some(Decimal::percent(75)),
        voting_period: Some(20000u64),
        effective_delay: Some(20000u64),
        proposal_deposit: Some(Uint128::new(123u128)),
        snapshot_period: Some(60u64),
        redemption_time: Some(1u64),
        poll_gas_limit: Some(2000000u64),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "addr0001".to_string(),
            prism_token: PRISM_TOKEN.to_string(),
            xprism_token: VOTING_TOKEN.to_string(),
            quorum: Decimal::percent(20),
            threshold: Decimal::percent(75),
            voting_period: 20000u64,
            effective_delay: 20000u64,
            proposal_deposit: Uint128::from(123u128),
            snapshot_period: 60u64,
            redemption_time: 1u64,
            poll_gas_limit: 2000000u64
        }
    );

    // Unauthorzied err
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        quorum: None,
        threshold: None,
        voting_period: None,
        effective_delay: None,
        proposal_deposit: None,
        snapshot_period: None,
        redemption_time: None,
        poll_gas_limit: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_abstain_votes_theshold() {
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time = env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).seconds();
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    const ALICE: &str = "alice";
    const ALICE_STAKE: u128 = 750_000_000_000u128;
    const BOB: &str = "bob";
    const BOB_STAKE: u128 = 250_000_000_000u128;
    const CINDY: &str = "cindy";
    const CINDY_STAKE: u128 = 260_000_000_000u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Alice stakes 750 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: ALICE.to_string(),
        amount: Uint128::from(ALICE_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Bob stakes 250 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: Uint128::from(BOB_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE + BOB_STAKE + CINDY_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128,
            ),
        )],
    )]);
    // Cindy stakes 260 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: CINDY.to_string(),
        amount: Uint128::from(CINDY_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Alice votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Abstain,
        amount: Uint128::from(ALICE_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::No,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
    // abstain votes should not affect threshold, so poll is passed
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );

    // invalid end poll - not in progress
    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, StdError::generic_err("Poll is not in progress"));

    // invalid cast vote - not in progress
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(CINDY, &[]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, StdError::generic_err("Poll is not in progress"));
}

#[test]
fn test_abstain_votes_quorum() {
    let mut deps = mock_dependencies(&[]);

    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time = env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).seconds();
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    const ALICE: &str = "alice";
    const ALICE_STAKE: u128 = 750_000_000_000u128;
    const BOB: &str = "bob";
    const BOB_STAKE: u128 = 50_000_000_000u128;
    const CINDY: &str = "cindy";
    const CINDY_STAKE: u128 = 20_000_000_000u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Alice stakes 750 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: ALICE.to_string(),
        amount: Uint128::from(ALICE_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Bob stakes 50 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: Uint128::from(BOB_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE + BOB_STAKE + CINDY_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128,
            ),
        )],
    )]);
    // Cindy stakes 50 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: CINDY.to_string(),
        amount: Uint128::from(CINDY_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Alice votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Abstain,
        amount: Uint128::from(ALICE_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    // abstain votes make the poll surpass quorum
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time = env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).seconds();
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Alice doesn't vote

    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 2,
        vote: VoteOption::Yes,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 2,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 2 };

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    // without abstain votes, quroum is not reached
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "2"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn snapshot_poll() {
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .seconds();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    //must not be executed
    let snapshot_err = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(
        StdError::generic_err("Cannot snapshot at this height",),
        snapshot_err
    );

    // change time
    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD - 10);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let fix_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap();

    assert_eq!(
        fix_res.attributes,
        vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", "1"),
            attr(
                "supply_snapshot",
                Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128).to_string()
            ),
        ]
    );

    //must not be executed
    let snapshot_error = execute(
        deps.as_mut(),
        creator_env,
        creator_info,
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(
        StdError::generic_err("Snapshot has already occurred"),
        snapshot_error
    );
}

#[test]
fn happy_days_cast_vote_with_snapshot() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(1, DEFAULT_VOTING_PERIOD, TEST_CREATOR, execute_res);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, execute_res);

    //cast_vote without snapshot
    let env = mock_env_height(0, 0);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let amount = 10u128;

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    // balance be double
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(22u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.supply_snapshot, None);
    let end_time = value.end_time;

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another voter cast a vote
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let env = mock_env_height(0, end_time - 9);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, amount, 1, VoteOption::Yes, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(
        value.supply_snapshot,
        Some(Uint128::new(22u128 + DEFAULT_PROPOSAL_DEPOSIT))
    );

    // snanpshot poll will not go through
    let snap_error = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(
        StdError::generic_err("Snapshot has already occurred"),
        snap_error
    );

    // increase supply
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(33u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    // another voter cast a vote but the snapshot is already occurred
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_3.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let env = mock_env_height(0, end_time - 8);
    let info = mock_info(TEST_VOTER_3, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_3, amount, 1, VoteOption::Yes, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(
        value.supply_snapshot,
        Some(Uint128::new(22u128 + DEFAULT_PROPOSAL_DEPOSIT))
    );
}

#[test]
fn fails_end_poll_quorum_inflation_without_snapshot_poll() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .seconds(),
        TEST_CREATOR,
        execute_res,
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(stake_amount, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, 0);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD - 10);

    // did not SnapshotPoll

    // staked amount get increased 10 times
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(((10 * stake_amount) + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(8 * stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _handle_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another voter cast a vote
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(creator_env.block.height, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER_2),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(10);

    // quorum must reach
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn happy_days_end_poll_with_controlled_quorum() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 100000000000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .seconds(),
        TEST_CREATOR,
        execute_res,
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(stake_amount, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", Uint128::from(stake_amount).to_string()),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD - 10);

    // send SnapshotPoll
    let fix_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap();

    assert_eq!(
        fix_res.attributes,
        vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", "1"),
            attr(
                "supply_snapshot",
                Uint128::new(stake_amount + DEFAULT_PROPOSAL_DEPOSIT).to_string()
            ),
        ]
    );

    // staked amount get increased 10 times
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(((10 * stake_amount) + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(8 * stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(8 * stake_amount),
    };
    let env = mock_env_height(creator_env.block.height, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", Uint128::from(8 * stake_amount).to_string()),
            attr("voter", TEST_VOTER_2),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(10);

    // quorum must reach
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.yes_votes.u128(), 9 * stake_amount);
}

#[test]
fn mint_xprism() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // start with 0 xprism supply
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::zero())],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))], // prism is already in contract balance when executing hook
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::MintXprism {}).unwrap(),
    });

    // attempt with wrong token
    let info = mock_info("test0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // also fails with xprism
    let info = mock_info(VOTING_TOKEN, &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("invalid cw20 hook message"));

    // right token
    let info = mock_info(PRISM_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "mint_xprism"),
            attr("mint_amount", "1000000")
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // supply of xprism is 1000000
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(1000000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(1000000u128 + 100u128 + 1000000u128), // first deposit + 100 reward + second deposit
            )],
        ),
    ]);

    // now exchange rate is different
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![attr("action", "mint_xprism"), attr("mint_amount", "999900")] // 1000000 * 1000000 / 1000100
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(999900u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn redeem_xprism() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // 1:1 exchange rate
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(1000000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(500000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });

    // attempt with wrong token
    let info = mock_info("test0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // also fails with prism
    let info = mock_info(PRISM_TOKEN, &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("invalid cw20 hook message"));

    // right token
    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "redeem_xprism"),
            attr("total_redeemed", "500000"),
            attr("prism_queued", "500000"),
        ]
    );
    assert!(res.messages.is_empty());

    // change exchange rate
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(1000000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(1000000u128 + 1000000u128), // add 1000000 rewards
            )],
        ),
    ]);

    // now exchange rate is different
    let res = query(deps.as_ref(), mock_env(), QueryMsg::XprismState {}).unwrap();
    let response: XprismStateResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        XprismStateResponse {
            exchange_rate: Decimal::from_ratio(500000u128 + 1000000u128, 500000u128),
            effective_xprism_supply: Uint128::from(500000u128),
            effective_underlying_prism: Uint128::from(500000u128 + 1000000u128),
            total_pending_withdraw_xprism: Uint128::from(500000u128),
            total_pending_withdraw_prism: Uint128::from(500000u128),
        }
    );

    // only one per block
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("can only execute one redeem_xprism operation per block")
    );
    // increase time
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(1);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "redeem_xprism"),
            attr("total_redeemed", "500000"),
            attr("prism_queued", "1500000"), // 500000 * (2000000 - 500000) / (1000000 - 500000)
        ]
    );
    assert!(res.messages.is_empty());
}

#[test]
fn query_prism_withdraw_orders() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // 1:1 exchange rate
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(1000000000000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(1000000000000u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let first_redeem_env = mock_env();
    let first_end_time = first_redeem_env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    let info = mock_info(VOTING_TOKEN, &[]);
    execute(deps.as_mut(), first_redeem_env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(200000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let mut second_redeem_env = mock_env();
    second_redeem_env.block.time = first_redeem_env.block.time.plus_seconds(50u64);
    let second_end_time = second_redeem_env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    execute(deps.as_mut(), second_redeem_env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(300000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let mut third_redeem_env = mock_env();
    third_redeem_env.block.time = second_redeem_env.block.time.plus_seconds(50u64);
    let third_end_time = third_redeem_env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    execute(deps.as_mut(), third_redeem_env, info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PrismWithdrawOrders {
            address: TEST_VOTER.to_string(),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PrismWithdrawOrdersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        PrismWithdrawOrdersResponse {
            claimable_amount: Uint128::zero(),
            orders: vec![
                (first_end_time, Uint128::from(100000u128)),
                (second_end_time, Uint128::from(200000u128)),
                (third_end_time, Uint128::from(300000u128)),
            ]
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PrismWithdrawOrders {
            address: TEST_VOTER.to_string(),
            start_after: Some(second_end_time),
            limit: Some(1u32),
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PrismWithdrawOrdersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        PrismWithdrawOrdersResponse {
            claimable_amount: Uint128::zero(),
            orders: vec![(first_end_time, Uint128::from(100000u128)),]
        }
    );

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(second_end_time + 1u64);
    let res = query(
        deps.as_ref(),
        env,
        QueryMsg::PrismWithdrawOrders {
            address: TEST_VOTER.to_string(),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PrismWithdrawOrdersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        PrismWithdrawOrdersResponse {
            claimable_amount: Uint128::from(300000u128), // claimable 1 and 2
            orders: vec![
                (first_end_time, Uint128::from(100000u128)),
                (second_end_time, Uint128::from(200000u128)),
                (third_end_time, Uint128::from(300000u128)),
            ]
        }
    );
}

#[test]
fn claim_redeemed_xprism() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // 1:1 exchange rate
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(1000000000000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(1000000000000u128),
            )],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let first_redeem_env = mock_env();
    let first_end_time = first_redeem_env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    let info = mock_info(VOTING_TOKEN, &[]);
    execute(deps.as_mut(), first_redeem_env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(200000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let mut second_redeem_env = mock_env();
    second_redeem_env.block.time = first_redeem_env.block.time.plus_seconds(50u64);
    let second_end_time = second_redeem_env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    execute(deps.as_mut(), second_redeem_env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(300000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let mut third_redeem_env = mock_env();
    third_redeem_env.block.time = second_redeem_env.block.time.plus_seconds(50u64);
    let third_end_time = third_redeem_env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    execute(deps.as_mut(), third_redeem_env, info, msg).unwrap();

    let msg = ExecuteMsg::ClaimRedeemedXprism {};
    let info = mock_info(TEST_VOTER, &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("nothing to claim"));

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(first_end_time + 1u64);
    let res = execute(deps.as_mut(), env, info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_redeemed_prism"),
            attr("prism_claimed", "100000"),
            attr("xprism_burned", "100000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(100000u128)
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: PRISM_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: TEST_VOTER.to_string(),
                    amount: Uint128::from(100000u128),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    // check deleted state
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PrismWithdrawOrders {
            address: TEST_VOTER.to_string(),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PrismWithdrawOrdersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        PrismWithdrawOrdersResponse {
            claimable_amount: Uint128::zero(),
            orders: vec![
                (second_end_time, Uint128::from(200000u128)),
                (third_end_time, Uint128::from(300000u128)),
            ]
        }
    );

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(third_end_time + 1u64);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_redeemed_prism"),
            attr("prism_claimed", "500000"),
            attr("xprism_burned", "500000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: PRISM_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: TEST_VOTER.to_string(),
                    amount: Uint128::from(500000u128),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_max_poll_votes() {
    const POLL_START_TIME: u64 = 1000;
    let stake_amount = 10000000000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());
    let creator_env = mock_env_height(0, POLL_START_TIME);
    let creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    for _ in 1..=MAX_POLL_VOTES_PER_USER + 1 {
        let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
        execute(
            deps.as_mut(),
            creator_env.clone(),
            creator_info.clone(),
            msg,
        )
        .unwrap();
    }

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(stake_amount, execute_res);

    for i in 1..=MAX_POLL_VOTES_PER_USER {
        let msg = ExecuteMsg::CastVote {
            poll_id: i as u64,
            vote: VoteOption::Yes,
            amount: Uint128::from(stake_amount),
        };
        let env = mock_env_height(0, POLL_START_TIME);
        let info = mock_info(TEST_VOTER, &[]);
        execute(deps.as_mut(), env, info, msg).unwrap();
    }

    let msg = ExecuteMsg::CastVote {
        poll_id: (MAX_POLL_VOTES_PER_USER + 1) as u64,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err,
        StdError::generic_err(format!("Can not vote on more than {} at the same time. Voting rewards of finished polls should be claimed.", 
        MAX_POLL_VOTES_PER_USER)));
}

#[test]
fn test_exchange_rate_after_claiming() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    mock_reply(deps.as_mut());

    // 1:1 exchange rate
    // user starts with 1M xprism (100K each)
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100000u128))],
        ),
    ]);

    // initial exchange rate is 1
    let res = query(deps.as_ref(), mock_env(), QueryMsg::XprismState {}).unwrap();
    let response: XprismStateResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        XprismStateResponse {
            exchange_rate: Decimal::one(),
            effective_xprism_supply: Uint128::from(100000u128),
            effective_underlying_prism: Uint128::from(100000u128),
            total_pending_withdraw_xprism: Uint128::zero(),
            total_pending_withdraw_prism: Uint128::zero(),
        }
    );

    // redeem 25K
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(25000u128),
        msg: to_binary(&Cw20HookMsg::RedeemXprism {}).unwrap(),
    });
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "redeem_xprism"),
            attr("total_redeemed", "25000"),
            attr("prism_queued", "25000"),
        ]
    );
    assert!(res.messages.is_empty());

    // exchange rate stays same after redeem, effective/pending values change
    let res = query(deps.as_ref(), mock_env(), QueryMsg::XprismState {}).unwrap();
    let response: XprismStateResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        XprismStateResponse {
            exchange_rate: Decimal::one(),
            effective_xprism_supply: Uint128::from(75000u128),
            effective_underlying_prism: Uint128::from(75000u128),
            total_pending_withdraw_xprism: Uint128::from(25000u128),
            total_pending_withdraw_prism: Uint128::from(25000u128),
        }
    );

    // change exchange rate
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(&TEST_VOTER.to_string(), &Uint128::from(100000u128))],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(100000u128 + 10000u128), // add 10K rewards
            )],
        ),
    ]);

    // query xprism state, exchange rate changes
    // (prism balance - pending prism withdraw) / (xprism supply - pending xprism burn)
    // (110K - 25K) / (100K - 25K) = 1.1333
    let res = query(deps.as_ref(), mock_env(), QueryMsg::XprismState {}).unwrap();
    let response: XprismStateResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        XprismStateResponse {
            exchange_rate: Decimal::from_ratio(85000u128, 75000u128),
            effective_xprism_supply: Uint128::from(75000u128),
            effective_underlying_prism: Uint128::from(85000u128),
            total_pending_withdraw_xprism: Uint128::from(25000u128),
            total_pending_withdraw_prism: Uint128::from(25000u128),
        }
    );

    // fast-forward by REDEMPTION_TIME, query withdraw orders,
    // we should have 25K claimable
    let mut env = mock_env();
    let claimable_time = env
        .block
        .time
        .plus_seconds(DEFAULT_REDEMPTION_TIME)
        .seconds();
    env.block.time = Timestamp::from_seconds(claimable_time + 1u64);
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::PrismWithdrawOrders {
            address: TEST_VOTER.to_string(),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PrismWithdrawOrdersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        PrismWithdrawOrdersResponse {
            claimable_amount: Uint128::from(25000u128),
            orders: vec![(claimable_time, Uint128::from(25000u128)),]
        }
    );

    // claim redeemed - 25K claimable
    let claim_msg = ExecuteMsg::ClaimRedeemedXprism {};
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info, claim_msg).unwrap();
    let claimable_amt = 25000u128;
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_redeemed_prism"),
            attr("prism_claimed", claimable_amt.to_string()),
            attr("xprism_burned", claimable_amt.to_string()),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: VOTING_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(claimable_amt)
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: PRISM_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: TEST_VOTER.to_string(),
                    amount: Uint128::from(claimable_amt),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    // simulate the above claim getting processed (25K) by adjusting the balances
    deps.querier.with_token_balances(&[
        (
            &VOTING_TOKEN.to_string(),
            &[(
                &TEST_VOTER.to_string(),
                &Uint128::from(100000u128 - 25000u128),
            )],
        ),
        (
            &PRISM_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &Uint128::from(100000u128 + 10000u128 - 25000u128),
            )],
        ),
    ]);

    // verify exchange rate and effective values didn't didn't change,
    // although pending did
    let res = query(deps.as_ref(), mock_env(), QueryMsg::XprismState {}).unwrap();
    let response: XprismStateResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        XprismStateResponse {
            exchange_rate: Decimal::from_ratio(85000u128, 75000u128),
            effective_xprism_supply: Uint128::from(75000u128),
            effective_underlying_prism: Uint128::from(85000u128),
            total_pending_withdraw_xprism: Uint128::zero(),
            total_pending_withdraw_prism: Uint128::zero(),
        }
    );
}
