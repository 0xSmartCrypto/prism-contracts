use crate::{
    contract::{execute, instantiate, query},
    error::ContractError,
    state::{RewardInfo, BOND_AMOUNTS, REWARD_INFO},
    vest::TIME_UNIT,
};
use cosmwasm_std::attr;
use cosmwasm_std::OwnedDeps;
use cosmwasm_std::{
    from_binary,
    testing::{mock_env, mock_info},
    to_binary, Addr, CosmosMsg, Decimal, OverflowError, OverflowOperation, Response, StdError,
    SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::{Asset, AssetInfo};
use integer_sqrt::IntegerSquareRoot;
use prism_common::testing::mock_querier::{mock_dependencies, MOCK_CONTRACT_ADDR};
use prism_protocol::gov::Cw20HookMsg as GovCw20HookMsg;
use prism_protocol::launch_pool::{
    ClaimType, ConfigResponse, Cw20HookMsg, DistributionInfo, DistributionStatusResponse,
    ExecuteMsg, InstantiateMsg, QueryMsg, RewardInfoResponse, VestingStatusResponse,
};
use prism_protocol::xprism_boost::Cw20HookMsg as BoostCw20HookMsg;
use prism_protocol::yasset_staking::{
    Cw20HookMsg as StakingHookMsg, ExecuteMsg as StakingExecuteMsg,
};
use std::collections::HashMap;

pub const DEFAULT_VESTING_PERIOD: u64 = 30 * TIME_UNIT;

// helper to compute the final vested time from the time of withdraw
fn compute_vested_time(withdraw_time: u64, vesting_period: u64) -> u64 {
    let mut vested_time = withdraw_time + vesting_period;
    vested_time -= vested_time % TIME_UNIT;
    vested_time
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    // invalid distribution schedule
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 99u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(1_000_u128),
    };

    let info = mock_info("addr0000", &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidDistributionSchedule {});

    // invalid boost distribution schedule
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 99u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(1_000_u128),
    };

    let info = mock_info("addr0000", &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidDistributionSchedule {});

    // successful init
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(1_000u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            operator: "op0000".to_string(),
            prism_token: "prism0000".to_string(),
            xprism_token: "xprism0000".to_string(),
            gov: "gov0000".to_string(),
            base_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
            boost_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
            boost_contract: "boost0000".to_string(),
            yluna_staking: "ylunastaking0000".to_string(),
            yluna_token: "ylunatoken0000".to_string(),
            vesting_period: DEFAULT_VESTING_PERIOD,
            min_bond_amount: Uint128::from(1_000u128),
        }
    );
}

#[test]
fn withdraw_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::zero(),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // bond
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // withdraw rewards after 50 seconds

    env.block.time = Timestamp::from_seconds(150u64);

    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![
                (vested_time, Uint128::from(500000u128)) // 1000000 / 2
            ],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(500000u128),
            total_weight: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(500000u128, 100u128),
        }
    );

    // withdraw rewards after 500 seconds (farming ended after 100 sec)
    env.block.time = Timestamp::from_seconds(600u64);

    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![(vested_time, Uint128::from(1000000u128))],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(1000000u128),
            total_weight: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(1000000u128, 100u128),
        }
    );
}

#[test]
fn withdraw_rewards_with_no_bond() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(1_u128),
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // withdraw rewards after 50 seconds
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(150u64);

    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(500000u128),
            total_weight: Uint128::zero(),
            pending_reward: Uint128::from(500000u128),
            reward_index: Decimal::zero(),
        }
    );

    // bond 100, verify pending_reward logic
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // withdraw immediately, we still get all the pending rewards
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![(vested_time, Uint128::from(500000u128))],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(500000u128),
            total_weight: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(500000u128, 100u128),
        }
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string()
                }
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(100u128),
            base_index: Decimal::from_ratio(500000u128, 100u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        }
    );
}

/// Test that withdraw_rewards_bulk returns error if called by non-owner.
#[test]
fn withdraw_rewards_bulk_auth() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: 100u64,
        min_bond_amount: Uint128::from(1_000_u128),
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::WithdrawRewardsBulk {
        limit: 2,
        start_after_address: Some(String::from("monkey")),
    };

    // Non-operator fails.
    let user_info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), user_info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Operator succeeds.
    let user_info = mock_info("op0000", &[]);
    execute(deps.as_mut(), mock_env(), user_info, msg).unwrap();
}

#[test]
fn withdraw_rewards_bulk() {
    // Summary of test:
    //  - Time 0: Init contract;
    //  - Time 1: Several people bond yluna before the event;
    //  - Time 10: Event starts;
    //  - Time 20: Event ends;
    //  - Time 21: Call withdraw_rewards_bulk a few times to process the entire
    //    list of people;
    //  - Assert that people's rewards are distributed to vesting schedules as
    //    expected.
    let mut deps = mock_dependencies(&[]);

    // Time 0: Init contract.
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        // Distribute 100 PRISMs between time 10s to 20s.
        base_distribution_schedule: (10u64, 20u64, Uint128::from(100u128)),
        boost_distribution_schedule: (10u64, 20u64, Uint128::zero()),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: 21 * TIME_UNIT,
        min_bond_amount: Uint128::from(1_u128),
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Time 1: Several people bond.
    for (person, amount_to_bond) in &[
        ("alice", 1),
        ("bob", 2),
        ("carol", 3),
        ("donald", 2),
        ("erika", 2),
    ] {
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1u64); // Before event starts.
        let info = mock_info("ylunatoken0000", &[]);
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: person.to_string(),
            amount: Uint128::from(*amount_to_bond as u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        });
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    }

    // Time 10: Event starts.
    // Time 20: Event ends.

    // Time 21: First call to withdraw_rewards_bulk.

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(21u64);
    let user_info = mock_info("op0000", &[]);
    let msg = ExecuteMsg::WithdrawRewardsBulk {
        limit: 2,
        start_after_address: Some(String::from("alice")), // Alice should be skipped.
    };
    let res = execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(res.messages, vec![],);
    assert_eq!(res.attributes, vec![attr("last_address", "carol")]);
    for (person, want_scheduled_vests) in vec![
        ("alice", vec![]),
        ("bob", vec![(1814400, Uint128::from(20u128))]),
        ("carol", vec![(1814400, Uint128::from(30u128))]),
        ("donald", vec![]),
        ("erika", vec![]),
        ("unrecognized", vec![]),
    ] {
        assert_eq!(
            from_binary::<VestingStatusResponse>(
                &query(
                    deps.as_ref(),
                    env.clone(),
                    QueryMsg::VestingStatus {
                        staker_addr: String::from(person),
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            VestingStatusResponse {
                scheduled_vests: want_scheduled_vests,
                withdrawable: Uint128::zero(),
            }
        );
    }
    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(100u128),
            total_weight: Uint128::from(10u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(100u128, 10u128),
        }
    );

    // Still time 21: Second call to withdraw_rewards_bulk.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(21u64); // After event ends.
    let user_info = mock_info("op0000", &[]);
    let msg = ExecuteMsg::WithdrawRewardsBulk {
        limit: 100, // Limit larger than actual amount of users should be fine.
        start_after_address: Some(String::from("carol")),
    };
    let res = execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(res.messages, vec![],);
    assert_eq!(res.attributes, vec![attr("last_address", "erika")]);
    for (person, want_scheduled_vests) in vec![
        ("alice", vec![]),
        ("bob", vec![(1814400, Uint128::from(20u128))]),
        ("carol", vec![(1814400, Uint128::from(30u128))]),
        ("donald", vec![(1814400, Uint128::from(20u128))]),
        ("erika", vec![(1814400, Uint128::from(20u128))]),
        ("unrecognized", vec![]),
    ] {
        assert_eq!(
            from_binary::<VestingStatusResponse>(
                &query(
                    deps.as_ref(),
                    env.clone(),
                    QueryMsg::VestingStatus {
                        staker_addr: String::from(person),
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            VestingStatusResponse {
                scheduled_vests: want_scheduled_vests,
                withdrawable: Uint128::zero(),
            }
        );
    }
    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(100u128),
            total_weight: Uint128::from(10u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(100u128, 10u128),
        }
    );

    // Still time 21: Last call to cover Alice.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(21u64);
    let user_info = mock_info("op0000", &[]);
    let msg = ExecuteMsg::WithdrawRewardsBulk {
        limit: 1,
        start_after_address: None, // None should default to Alice since she's first alphabetically.
    };
    let res = execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(res.messages, vec![],);
    assert_eq!(res.attributes, vec![attr("last_address", "alice")]);
    for (person, want_scheduled_vests) in vec![
        ("alice", vec![(1814400, Uint128::from(10u128))]),
        ("bob", vec![(1814400, Uint128::from(20u128))]),
        ("carol", vec![(1814400, Uint128::from(30u128))]),
        ("donald", vec![(1814400, Uint128::from(20u128))]),
        ("erika", vec![(1814400, Uint128::from(20u128))]),
        ("unrecognized", vec![]),
    ] {
        assert_eq!(
            from_binary::<VestingStatusResponse>(
                &query(
                    deps.as_ref(),
                    env.clone(),
                    QueryMsg::VestingStatus {
                        staker_addr: String::from(person),
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            VestingStatusResponse {
                scheduled_vests: want_scheduled_vests,
                withdrawable: Uint128::zero(),
            }
        );
    }
    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(100u128),
            total_weight: Uint128::from(10u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(100u128, 10u128),
        }
    );
}

#[test]
fn bond() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (
            100000000000000u64,
            200000000000000u64,
            Uint128::from(1000000u128),
        ),
        boost_distribution_schedule: (0u64, 10u64, Uint128::zero()),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100_u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(99u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // Sad path: wrong token
    let info = mock_info("lp00001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Sad path: correct token, but too small of an amount (below min_bond_amount threshold).
    let info = mock_info("ylunatoken0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidBond {
            reason: "bond amount too low; must be at least 100".to_string()
        }
    );

    // Happy path: correct token and amount above min_bond_amount threshold.
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("ylunatoken0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "ylunatoken0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "ylunastaking0000".to_string(),
                amount: Uint128::from(100u128),
                msg: to_binary(&StakingHookMsg::Bond {}).unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "addr0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(100u128),
            base_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            boost_weight: Uint128::zero(),
            active_boost: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::zero(),
            total_weight: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::zero(),
        }
    );
}

#[test]
fn unbond() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        // Distribute 5000 reward tokens during a 30-second event.
        base_distribution_schedule: (30u64, 60u64, Uint128::from(5_000u128)),
        boost_distribution_schedule: (0u64, 10u64, Uint128::zero()),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(99_u128),
    };

    let info = mock_info("addr0000", &[]);
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // bond 100 right at the beginning of the event.
    env.block.time = Timestamp::from_seconds(30);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // try to unbond from a different sender with nothing bonded
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(101u128)),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(
        res,
        ContractError::InvalidUnbond {
            reason: "no tokens bonded".to_string()
        }
    );

    // try to unbond more than we've bonded
    let unbond_amt = Uint128::from(101u128);
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(unbond_amt),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(
        res,
        ContractError::InvalidUnbond {
            reason: "can not unbond more than the bonded amount".to_string()
        }
    );

    // Successful unbond of 25 at end of event.
    env.block.time = Timestamp::from_seconds(60);
    let unbond_amt = Uint128::from(25u128);
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(unbond_amt),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ylunastaking0000".to_string(),
                msg: to_binary(&StakingExecuteMsg::Unbond {
                    amount: Some(unbond_amt),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ylunatoken0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0000".to_string(),
                    amount: unbond_amt,
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(5_000u128),
            total_weight: Uint128::from(75u128),
            pending_reward: Uint128::zero(),
            // reward_index is 5000/100 because 5000 thousand PRISM tokens where
            // distributed during the event among 100 bound yluna tokens.
            reward_index: Decimal::from_ratio(5_000u128, 100u128),
        }
    );

    // Internal assertion: make sure user has entries in these maps.
    REWARD_INFO
        .load(&deps.storage, "addr0000".as_bytes())
        .unwrap();
    BOND_AMOUNTS
        .load(&deps.storage, "addr0000".as_bytes())
        .unwrap();

    // successful unbond of remaining 75 (using None as amount)
    let remaining_amt = Uint128::from(75u128);
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond { amount: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ylunastaking0000".to_string(),
                msg: to_binary(&StakingExecuteMsg::Unbond {
                    amount: Some(remaining_amt),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ylunatoken0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0000".to_string(),
                    amount: remaining_amt,
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "addr0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap()
        .base,
        DistributionInfo {
            total_distributed: Uint128::from(5_000u128),
            total_weight: Uint128::zero(),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(5_000u128, 100u128),
        }
    );

    // At this point the user has unbound everything they had, but they should
    // still have pending_reward > 0 because they haven't withdrawn those
    // rewards yet.
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: "addr0000".to_string()
                }
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::zero(),
            base_index: Decimal::from_ratio(5_000u128, 100u128),
            pending_reward: Uint128::from(5_000u128),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        }
    );

    // Internal assertion: make sure user has entries in these maps, even after
    // unbonding everything.
    REWARD_INFO
        .load(&deps.storage, "addr0000".as_bytes())
        .unwrap();
    BOND_AMOUNTS
        .load(&deps.storage, "addr0000".as_bytes())
        .unwrap();
}

#[test]
fn claim_withdrawn_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100_u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // bond
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // withdraw rewards after 50 seconds

    env.block.time = Timestamp::from_seconds(150u64);

    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // try to claim before claim end_time expires
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Prism,
    };
    let err = execute(deps.as_mut(), env.clone(), user_info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidClaimWithdrawnRewards {
            reason: "There are no claimable rewards".to_string()
        }
    );

    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![
                (vested_time, Uint128::from(500000u128)) // 1000000 / 2
            ],
            withdrawable: Uint128::zero(),
        }
    );

    env.block.time = Timestamp::from_seconds(vested_time + 1);

    // verify query works after vesting period ends
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![
                (vested_time, Uint128::from(500000u128)) // 1000000 / 2
            ],
            withdrawable: Uint128::from(500000u128),
        }
    );

    let res = execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: Uint128::from(500000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // vest record removed
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![],
            withdrawable: Uint128::zero(),
        }
    );
}

#[test]
fn admin_withdraw_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(1_000_u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::AdminWithdrawRewards {};

    // wrong adddress attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    deps.querier.with_token_balances(&[
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100u128))],
        ),
        (
            &"pluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(200u128))],
        ),
    ]);

    // correct address
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ylunastaking0000".to_string(),
                msg: to_binary(&StakingExecuteMsg::ClaimRewards {}).unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::AdminSendWithdrawnRewards {
                    original_balances: vec![
                        Asset {
                            info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                            amount: Uint128::from(100u128),
                        },
                        Asset {
                            info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                            amount: Uint128::from(200u128),
                        },
                        Asset {
                            info: AssetInfo::Native("uluna".to_string()),
                            amount: Uint128::from(0u128),
                        }
                    ],
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // now call the hook
    let msg = ExecuteMsg::AdminSendWithdrawnRewards {
        original_balances: vec![
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                amount: Uint128::from(200u128),
            },
        ],
    };

    // simulate that the contract received rewards after claiming
    deps.querier.with_token_balances(&[
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(250u128))],
        ),
        (
            &"pluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(400u128))],
        ),
    ]);

    // wrong adddress attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // correct address
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "owner0000".to_string(),
                    amount: Uint128::from(150u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "pluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "owner0000".to_string(),
                    amount: Uint128::from(200u128),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // call the hook again with same balances, no messages expected
    let msg = ExecuteMsg::AdminSendWithdrawnRewards {
        original_balances: vec![
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                amount: Uint128::from(250u128),
            },
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                amount: Uint128::from(400u128),
            },
        ],
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages, vec![])
}

/// This test is a nightmarish long scenario that excercises the tricky
/// rewards_index logic. I sleep better knowing that this exists, but I have
/// panic attacks thinking of updating this in the future.
#[test]
fn rewards_index_from_hell() {
    // Summary of test:
    //  - Time 0: Init contract;
    //  - Time 100: Rewards event starts (no one has bonded anything yet);
    //  - Time 110: Alice and Bob bond something on exactly the same block;
    //  - Time 120: Alice unbonds part of her position;
    //  - Time 130: Bob withdraws his rewards on his own;
    //  - Time 140: Carol bonds something;
    //  - Time 150: Rewards for Alice are withdrawn by the bot;
    //  - Time 160: Carol bonds a lot more;
    //  - Time 200: Event ends;
    //  - Time 210: Carol unbonds all of her position;
    //  - Time 300: Rewards for all users are withdrawn by the bot;

    // Helper functions to keep actual test below more concise. This test is
    // already unwieldy enough; without these helpers it would be pure torture.
    let bond = |time: u64, deps: &mut OwnedDeps<_, _, _>, who: &str, amount_to_bond: u128| {
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(time);
        let info = mock_info("ylunatoken0000", &[]);
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: who.to_string(),
            amount: Uint128::from(amount_to_bond),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        });
        execute(deps.as_mut(), env, info, msg).unwrap();
    };
    let unbond = |time: u64, deps: &mut OwnedDeps<_, _, _>, who: &str, amount_to_unbond: u128| {
        let info = mock_info(who, &[]);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(time);
        let msg = ExecuteMsg::Unbond {
            amount: Some(Uint128::from(amount_to_unbond)),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();
    };
    let check_global_distribution_status =
        |time: u64, deps: &OwnedDeps<_, _, _>, want_distribution_status: DistributionInfo| {
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(time);
            assert_eq!(
                from_binary::<DistributionStatusResponse>(
                    &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
                )
                .unwrap()
                .base,
                want_distribution_status,
            );
        };
    let check_individual_reward_info =
        |time: u64, deps: &OwnedDeps<_, _, _>, who: &str, want_reward_info: RewardInfo| {
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(time);
            assert_eq!(
                from_binary::<RewardInfo>(
                    &query(
                        deps.as_ref(),
                        env,
                        QueryMsg::RewardInfo {
                            staker_addr: String::from(who)
                        },
                    )
                    .unwrap(),
                )
                .unwrap(),
                want_reward_info,
            );
        };
    let check_individual_vesting_schedule =
        |time: u64,
         deps: &OwnedDeps<_, _, _>,
         who: &str,
         want_scheduled_vests: Vec<(u64, Uint128)>| {
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(time);
            assert_eq!(
                from_binary::<VestingStatusResponse>(
                    &query(
                        deps.as_ref(),
                        env,
                        QueryMsg::VestingStatus {
                            staker_addr: String::from(who),
                        },
                    )
                    .unwrap(),
                )
                .unwrap(),
                VestingStatusResponse {
                    scheduled_vests: want_scheduled_vests,
                    withdrawable: Uint128::zero(),
                }
            );
        };
    let check_bond_balances = |deps: &OwnedDeps<_, _, _>, want_balances: Vec<(&str, usize)>| {
        for (who, want_balance) in want_balances {
            let got_balance = BOND_AMOUNTS
                .load(deps.as_ref().storage, String::from(who).as_bytes())
                .unwrap();
            assert_eq!(got_balance, Uint128::from(want_balance as u128),)
        }
    };

    // Actual test starts here.

    // Time 0: Init contract and start rewards event before anyone bonds
    // anything.
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        // Distribute 100k PRISMs between time 100s to 200s.
        base_distribution_schedule: (100u64, 200u64, Uint128::from(100_000u128)),
        boost_distribution_schedule: (0u64, 10u64, Uint128::zero()),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: 21 * TIME_UNIT,
        min_bond_amount: Uint128::from(1_u128),
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Time 110: No one has bonded anything yet. Rewards emitted since time 100
    // get accumulated for a lucky future winner in the pending_reward field.
    check_global_distribution_status(
        110,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(10_000u128),
            total_weight: Uint128::from(0u128),
            pending_reward: Uint128::from(10_000u128),
            reward_index: Decimal::zero(),
        },
    );

    // Still time 110: Alice and Bob bond something on exactly the same block
    // 10s after event started. Bob comes first on the block so he will
    // eventually take all of the reward for the first 10s.
    bond(110, &mut deps, "bob", 2);
    check_bond_balances(&deps, vec![("bob", 2)]);
    check_global_distribution_status(
        110,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(10_000u128),
            total_weight: Uint128::from(2u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128),
        },
    );
    check_individual_reward_info(
        110,
        &deps,
        "bob",
        RewardInfo {
            base_index: Decimal::zero(), // this index == 0 is what will allow Bob to claim the initial rewards. Notice that Alice's index field below is != 0.
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(110, &deps, "bob", vec![]);
    bond(110, &mut deps, "alice", 3);
    check_bond_balances(&deps, vec![("alice", 3), ("bob", 2)]);
    check_global_distribution_status(
        110,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(10_000u128),
            total_weight: Uint128::from(5u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128),
        },
    );
    check_individual_reward_info(
        110,
        &deps,
        "alice",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(110, &deps, "alice", vec![]);

    // Time 120: Alice unbonds part of her position.
    unbond(120, &mut deps, "alice", 1);
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2)]);
    check_global_distribution_status(
        120,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(20_000u128),
            total_weight: Uint128::from(4u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128),
        },
    );
    check_individual_reward_info(
        120,
        &deps,
        "alice",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128),
            pending_reward: Uint128::from(10_000 / 5 * 3_u128),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(120, &deps, "alice", vec![]);

    //  Time 130: Bob withdraws his rewards.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(130);
    let info = mock_info("bob", &[]);
    execute(deps.as_mut(), env, info, ExecuteMsg::WithdrawRewards {}).unwrap();
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2)]);
    check_global_distribution_status(
        130,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(30_000u128),
            total_weight: Uint128::from(4u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128),
        },
    );
    check_individual_reward_info(
        130,
        &deps,
        "bob",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(
        130,
        &deps,
        "bob",
        vec![(
            1814400,
            Uint128::from(10_000 + 10_000 / 5 * 2 + 10_000 / 4 * 2_u128),
        )],
    );

    // Time 140: Carol bonds something.
    bond(140, &mut deps, "carol", 4);
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2), ("carol", 4)]);
    check_global_distribution_status(
        140,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(40_000u128),
            total_weight: Uint128::from(8u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128),
        },
    );
    check_individual_reward_info(
        140,
        &deps,
        "carol",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(140, &deps, "carol", vec![]);

    // Time 150: Rewards for Alice are withdrawn by the bot.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(150);
    let info = mock_info("op0000", &[]);
    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::WithdrawRewardsBulk {
            limit: 1,
            start_after_address: None, // None means first address, which happens to be Alice.
        },
    )
    .unwrap();
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2), ("carol", 4)]);
    check_global_distribution_status(
        150,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(50_000u128),
            total_weight: Uint128::from(8u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128),
        },
    );
    check_individual_reward_info(
        150,
        &deps,
        "alice",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(
        150,
        &deps,
        "alice",
        vec![(
            1814400,
            Uint128::from(10_000 / 5 * 3 + 10_000 / 4 * 2 + 10_000 / 4 * 2 + 10_000 / 8 * 2_u128),
        )],
    );

    // Time 160: Carol bonds a lot more.
    bond(160, &mut deps, "carol", 42);
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2), ("carol", 46)]);
    check_global_distribution_status(
        160,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(60_000u128),
            total_weight: Uint128::from(50u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128),
        },
    );
    check_individual_reward_info(
        160,
        &deps,
        "carol",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128),
            pending_reward: Uint128::from(10_000 / 8 * 4 + 10_000 / 8 * 4_u128),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(160, &deps, "carol", vec![]);

    // Time 200: Event ends. Don't need to do anything.

    // Time 210: Carol unbonds all of her position;
    unbond(210, &mut deps, "carol", 46);
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2), ("carol", 0)]);
    check_global_distribution_status(
        210,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(100_000u128),
            total_weight: Uint128::from(4u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(40_000u128, 50u128),
        },
    );
    check_individual_reward_info(
        210,
        &deps,
        "carol",
        RewardInfo {
            base_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(40_000u128, 50u128),
            pending_reward: Uint128::from(10_000 / 8 * 4 + 10_000 / 8 * 4 + 40_000 / 50 * 46_u128),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(210, &deps, "carol", vec![]);

    // Time 300: Rewards for all users are withdrawn by the bot.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(300);
    let info = mock_info("op0000", &[]);
    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::WithdrawRewardsBulk {
            limit: 100, // Everyone.
            start_after_address: None,
        },
    )
    .unwrap();

    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2), ("carol", 0)]);
    check_global_distribution_status(
        300,
        &deps,
        DistributionInfo {
            total_distributed: Uint128::from(100_000u128),
            total_weight: Uint128::from(4u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(40_000u128, 50u128),
        },
    );
    for person in &["alice", "bob", "carol"] {
        check_individual_reward_info(
            300,
            &deps,
            person,
            RewardInfo {
                base_index: Decimal::from_ratio(10_000u128, 2u128)
                    + Decimal::from_ratio(10_000u128, 5u128)
                    + Decimal::from_ratio(10_000u128, 4u128)
                    + Decimal::from_ratio(10_000u128, 4u128)
                    + Decimal::from_ratio(10_000u128, 8u128)
                    + Decimal::from_ratio(10_000u128, 8u128)
                    + Decimal::from_ratio(40_000u128, 50u128),
                pending_reward: Uint128::zero(),
                boost_index: Decimal::zero(),
                active_boost: Uint128::zero(),
                boost_weight: Uint128::zero(),
            },
        );
    }
    for (person, will_vest_balance) in &[
        (
            "alice",
            10_000 / 5 * 3
                + 10_000 / 4 * 2
                + 10_000 / 4 * 2
                + 10_000 / 8 * 2
                + 10_000 / 8 * 2
                + 40_000 / 50 * 2,
        ),
        (
            "bob",
            10_000 // Early bird reward.
                + 10_000 / 5 * 2
                + 10_000 / 4 * 2
                + 10_000 / 4 * 2
                + 10_000 / 8 * 2
                + 10_000 / 8 * 2
                + 40_000 / 50 * 2,
        ),
        ("carol", 10_000 / 8 * 4 + 10_000 / 8 * 4 + 40_000 / 50 * 46),
    ] {
        check_individual_vesting_schedule(
            300,
            &deps,
            person,
            vec![(1814400, Uint128::from(*will_vest_balance as u128))],
        );
    }
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(&[]);

    // successful init
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(1_000u128),
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_binary::<ConfigResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()
        )
        .unwrap(),
        ConfigResponse {
            owner: "owner0000".to_string(),
            operator: "op0000".to_string(),
            prism_token: "prism0000".to_string(),
            xprism_token: "xprism0000".to_string(),
            gov: "gov0000".to_string(),
            base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            boost_contract: "boost0000".to_string(),
            yluna_staking: "ylunastaking0000".to_string(),
            yluna_token: "ylunatoken0000".to_string(),
            vesting_period: DEFAULT_VESTING_PERIOD,
            min_bond_amount: Uint128::from(1_000u128),
        }
    );

    // Sad path: Unauthorized.
    let msg = ExecuteMsg::UpdateConfig {
        min_bond_amount: Some(Uint128::from(222u128)),
    };
    let info = mock_info("eve0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Happy path: Providing None as input doesn't mutate anything.
    let msg = ExecuteMsg::UpdateConfig {
        min_bond_amount: None,
    };
    let info = mock_info("owner0000", &[]);
    assert_eq!(
        execute(deps.as_mut(), mock_env(), info, msg).unwrap(),
        Response::new(),
    );
    assert_eq!(
        from_binary::<ConfigResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()
        )
        .unwrap(),
        ConfigResponse {
            owner: "owner0000".to_string(),
            operator: "op0000".to_string(),
            prism_token: "prism0000".to_string(),
            xprism_token: "xprism0000".to_string(),
            gov: "gov0000".to_string(),
            base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            boost_contract: "boost0000".to_string(),
            yluna_staking: "ylunastaking0000".to_string(),
            yluna_token: "ylunatoken0000".to_string(),
            vesting_period: DEFAULT_VESTING_PERIOD,
            min_bond_amount: Uint128::from(1_000u128),
        }
    );

    // // Happy path: Providing Some as input does mutate things.
    let msg = ExecuteMsg::UpdateConfig {
        min_bond_amount: Some(Uint128::from(222_u128)),
    };
    let info = mock_info("owner0000", &[]);
    assert_eq!(
        execute(deps.as_mut(), mock_env(), info, msg).unwrap(),
        Response::new(),
    );
    assert_eq!(
        from_binary::<ConfigResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()
        )
        .unwrap(),
        ConfigResponse {
            owner: "owner0000".to_string(),
            operator: "op0000".to_string(),
            prism_token: "prism0000".to_string(),
            xprism_token: "xprism0000".to_string(),
            gov: "gov0000".to_string(),
            base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            boost_contract: "boost0000".to_string(),
            yluna_staking: "ylunastaking0000".to_string(),
            yluna_token: "ylunatoken0000".to_string(),
            vesting_period: DEFAULT_VESTING_PERIOD,
            min_bond_amount: Uint128::from(222_u128),
        }
    );
}

#[test]
fn test_claim_withdrawn_rewards_as_xprism() {
    let mut deps = mock_dependencies(&[]);
    // successful init
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // bond
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // withdraw rewards 50 seconds after event starts
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // claim as xprism after vesting period
    // we create one message:
    // 1 - gov MintXprism using sender as receiver
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Xprism,
    };
    env.block.time = Timestamp::from_seconds(vested_time + 1);
    let res = execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "gov0000".to_string(),
                amount: Uint128::from(500000u128),
                msg: to_binary(&GovCw20HookMsg::MintXprism {
                    receiver: Some(user_info.sender.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_withdrawn_rewards"),
            attr("claim_type", "Xprism"),
            attr("prism_reward_claimed", 500000u128.to_string()),
        ]
    );

    // verify vest record removed
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: user_info.sender.to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![],
            withdrawable: Uint128::zero(),
        }
    );

    // verify pending reward 0
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: user_info.sender.to_string()
                }
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(100u128),
            base_index: Decimal::from_ratio(500_000u128, 100u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        }
    );
}

#[test]
fn test_claim_withdrawn_rewards_as_amps() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // bond
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // withdraw rewards after 50 seconds
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // claim as amps after vesting period
    // we create two messages:
    // 1 - gov MintXprism using contract address as receiver
    // 2 - BondWithBoostContractHook
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Amps,
    };
    env.block.time = Timestamp::from_seconds(vested_time + 1);
    let res = execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "gov0000".to_string(),
                amount: Uint128::from(500000u128),
                msg: to_binary(&GovCw20HookMsg::MintXprism {
                    receiver: Some(env.contract.address.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BondWithBoostContractHook {
                receiver: user_info.clone().sender,
                prev_xprism_balance: Uint128::zero(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_withdrawn_rewards"),
            attr("claim_type", "Amps"),
            attr("prism_reward_claimed", 500000u128.to_string()),
        ]
    );

    // verify vest record removed
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: user_info.sender.to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![],
            withdrawable: Uint128::zero(),
        }
    );

    // verify pending reward 0
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: user_info.sender.to_string()
                }
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(100u128),
            base_index: Decimal::from_ratio(500_000u128, 100u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        }
    );
}

/// Same as test_claim_withdrawn_rewards_as_amps, except in this test the
/// launch-pool contract has an initial xPRISM balance.  Therefore
/// the BondWithBoostContractHook should set the prev_xprism_balance field
/// to the user's original balance, which prevents the hook from spending
/// that initial balance.
#[test]
fn test_claim_withdrawn_rewards_as_amps_with_xprism_balance() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // bond
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // withdraw rewards after 50 seconds
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // add some xprism balance and claim as amps after vesting period
    // we create two messages:
    // 1 - gov MintXprism using contract address as receiver
    // 2 - BondWithBoostContractHook
    deps.querier.with_token_balances(&[(
        &"xprism0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000u128))],
    )]);
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Amps,
    };
    env.block.time = Timestamp::from_seconds(vested_time + 1);
    let res = execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "gov0000".to_string(),
                amount: Uint128::from(500000u128),
                msg: to_binary(&GovCw20HookMsg::MintXprism {
                    receiver: Some(env.contract.address.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::BondWithBoostContractHook {
                receiver: user_info.sender.clone(),
                prev_xprism_balance: Uint128::from(1000u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_withdrawn_rewards"),
            attr("claim_type", "Amps"),
            attr("prism_reward_claimed", 500000u128.to_string()),
        ]
    );

    // verify vest record removed
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: user_info.sender.to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![],
            withdrawable: Uint128::zero(),
        }
    );

    // verify pending reward 0
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: user_info.sender.to_string()
                }
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(100u128),
            base_index: Decimal::from_ratio(500_000u128, 100u128),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            active_boost: Uint128::zero(),
            boost_weight: Uint128::zero(),
        }
    );
}

#[test]
fn test_bond_with_boost_contract_hook() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::BondWithBoostContractHook {
        receiver: Addr::unchecked("addr0000"),
        prev_xprism_balance: Uint128::zero(),
    };

    // unauthorized - only contract can execute
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // change info to contract addr, required for auth
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);

    // create some xprism balance on the contract
    deps.querier.with_token_balances(&[(
        &"xprism0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000u128))],
    )]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "xprism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "boost0000".to_string(),
                amount: Uint128::from(1000u128),
                msg: to_binary(&BoostCw20HookMsg::Bond {
                    user: Some("addr0000".to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond_with_boost_contract_hook"),
            attr("bond_amount", 1000u128.to_string()),
        ]
    );

    // call hook with prev balance of 250, we should only bond 750 here
    let msg = ExecuteMsg::BondWithBoostContractHook {
        receiver: Addr::unchecked("addr0000"),
        prev_xprism_balance: Uint128::from(250u128),
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "xprism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "boost0000".to_string(),
                amount: Uint128::from(750u128),
                msg: to_binary(&BoostCw20HookMsg::Bond {
                    user: Some("addr0000".to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond_with_boost_contract_hook"),
            attr("bond_amount", 750u128.to_string()),
        ]
    );

    // call hook with prev balance of >1000, which should fail.
    let msg = ExecuteMsg::BondWithBoostContractHook {
        receiver: Addr::unchecked("addr0000"),
        prev_xprism_balance: Uint128::from(1500u128),
    };
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::from(StdError::Overflow {
            source: OverflowError {
                operation: OverflowOperation::Sub,
                operand1: "1000".to_string(),
                operand2: "1500".to_string()
            }
        })
    );

    // reset balance to 0
    deps.querier.with_token_balances(&[(
        &"xprism0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::zero())],
    )]);
    let msg = ExecuteMsg::BondWithBoostContractHook {
        receiver: Addr::unchecked("addr0000"),
        prev_xprism_balance: Uint128::zero(),
    };
    // no messagse sent here due to a balance of 0
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 0);
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond_with_boost_contract_hook"),
            attr("bond_amount", 0u128.to_string()),
        ]
    );
}

#[test]
fn test_activate_boost_single_user() {
    // Summary of test:
    //  T=90 - Alice bonds 100
    //  T=90 - Alice activates her boost
    //  T=150 - Alice withdraws her rewards
    //  T=vested - Alice claims rewards after vesting period
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // set alice's boost
    let mut boost_map = HashMap::new();
    boost_map.insert("alice0000".to_string(), Uint128::from(50u128));
    deps.querier.with_boost_querier(boost_map);

    // alice bonds 100
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice activates boost immediately
    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("alice0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice withdraw rewards after 50 seconds, the entire boost pool is hers
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // claim rewards for alice's withdraw event
    let total_base_reward = 500_000u128;
    let total_boost_reward = 500_000u128;
    let alice_bonded = 100u128;
    let alice_boost_value = 50u128;
    let total_bonded = alice_bonded;
    let alice_boost_weight = (alice_bonded * alice_boost_value).integer_sqrt();
    let total_boost_weight = alice_boost_weight;
    let alice_base_reward = Uint128::from(total_base_reward) * Uint128::from(alice_bonded)
        / Uint128::from(total_bonded);
    let alice_boost_reward = Uint128::from(total_boost_reward)
        * Decimal::from_ratio(alice_boost_weight, total_boost_weight);
    let alice_total_reward = alice_base_reward + alice_boost_reward - Uint128::from(1u128); // subtract 1 due to rounding

    // alice claims rewards after vesting period
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Prism,
    };
    env.block.time = Timestamp::from_seconds(vested_time + 1);
    let res = execute(deps.as_mut(), env, user_info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: alice_total_reward,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(alice_bonded),
            base_index: Decimal::from_ratio(total_base_reward, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::from_ratio(total_boost_reward, total_boost_weight),
            boost_weight: Uint128::from(alice_boost_weight),
            active_boost: Uint128::from(alice_boost_value),
        }
    );
}

#[test]
fn test_activate_boost_multi_user() {
    // Summary of test:
    //  T=90 - Alice bonds 100, Bob bonds 500
    //  T=90 - Alice and Bob both activate their boost
    //  T=150 - Alice and Bob both withdraw rewards
    //  T=vested - Alice and Bob both claim rewards after vesting period
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // set alice's and bob's boost
    let mut boost_map = HashMap::new();
    boost_map.insert("alice0000".to_string(), Uint128::from(50u128));
    boost_map.insert("bob0000".to_string(), Uint128::from(50u128));
    deps.querier.with_boost_querier(boost_map);

    // alice bonds 100
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice activates boost immediately
    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("alice0000", &[]);
    execute(deps.as_mut(), env, info, msg).unwrap();

    // bob bonds 500
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "bob0000".to_string(),
        amount: Uint128::from(500u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // bob activates boost immediately
    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("bob0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice withdraw rewards at T=150
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    // bob withdraw rewards at T=150
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("bob0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    // both alice and bob will vest at vested_time1
    let vested_time1 = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // claim rewards for alice and bob's first withdraw event
    let total_base_reward = 500_000u128;
    let total_boost_reward = 500_000u128;
    let alice_bonded = 100u128;
    let alice_boost_value = 50u128;
    let bob_bonded = 500u128;
    let bob_boost_value = 50u128;
    let total_bonded = alice_bonded + bob_bonded;
    let alice_boost_weight = (alice_bonded * alice_boost_value).integer_sqrt();
    let bob_boost_weight = (bob_bonded * bob_boost_value).integer_sqrt();
    let total_boost_weight = alice_boost_weight + bob_boost_weight;
    let alice_base_reward = Uint128::from(total_base_reward) * Uint128::from(alice_bonded)
        / Uint128::from(total_bonded);
    let alice_boost_reward = Uint128::from(total_boost_reward)
        * Decimal::from_ratio(alice_boost_weight, total_boost_weight);
    let alice_total_reward = alice_base_reward + alice_boost_reward;
    let bob_base_reward =
        Uint128::from(total_base_reward) * Uint128::from(bob_bonded) / Uint128::from(total_bonded);
    let bob_boost_reward = Uint128::from(total_boost_reward)
        * Decimal::from_ratio(bob_boost_weight, total_boost_weight);
    let bob_total_reward = bob_base_reward + bob_boost_reward;

    // alice claims rewards after vesting period
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Prism,
    };
    env.block.time = Timestamp::from_seconds(vested_time1 + 1);
    let res = execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: alice_total_reward,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(alice_bonded),
            base_index: Decimal::from_ratio(total_base_reward, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::from_ratio(total_boost_reward, total_boost_weight),
            boost_weight: Uint128::from(alice_boost_weight),
            active_boost: Uint128::from(alice_boost_value),
        }
    );

    // bob claims rewards after vesting period
    let user_info = mock_info("bob0000", &[]);
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Prism,
    };
    env.block.time = Timestamp::from_seconds(vested_time1 + 1);
    let res = execute(deps.as_mut(), env, user_info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "bob0000".to_string(),
                amount: bob_total_reward,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "bob0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(bob_bonded),
            base_index: Decimal::from_ratio(total_base_reward, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::from_ratio(total_boost_reward, total_boost_weight),
            boost_weight: Uint128::from(bob_boost_weight),
            active_boost: Uint128::from(bob_boost_value),
        }
    );
}

#[test]
fn test_activate_boost_multi_user_two_intervals() {
    //  T=90 - Alice bonds 100, Bob bonds 500
    //  T=90 - Alice and Bob both activate their boost
    //  T=150 - Alice and Bob both withdraw rewards
    //  T=150 - global boost value changes to 75, but only Alice reactivates boost
    //  T=200 - Alice and Bob both withdraw rewards again (same vest timestamp
    //          as prior withdraw due to time-unit snap-back logic)
    //  T=vested - Alice and Bob both claim rewards after vesting period.  Alice's
    //          percentage of the total reward increases due to her larger boost
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // set alice's and bob's boost
    let mut boost_map = HashMap::new();
    boost_map.insert("alice0000".to_string(), Uint128::from(50u128));
    boost_map.insert("bob0000".to_string(), Uint128::from(50u128));
    deps.querier.with_boost_querier(boost_map.clone());

    // alice bonds 100
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice activates boost immediately
    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("alice0000", &[]);
    execute(deps.as_mut(), env, info, msg).unwrap();

    // bob bonds 500
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "bob0000".to_string(),
        amount: Uint128::from(500u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // bob activates boost immediately
    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("bob0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice withdraw rewards at T=150
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    // bob withdraw rewards at T=150
    env.block.time = Timestamp::from_seconds(150u64);
    let user_info = mock_info("bob0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    // for first withdraws, both alice and bob will vest at vested_time1
    let vested_time1 = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);

    // for the second half of the reward interval, change Alice's boost value to 75
    boost_map.insert("alice0000".to_string(), Uint128::from(75u128));
    deps.querier.with_boost_querier(boost_map);

    // only alice re-activates her boost
    env.block.time = Timestamp::from_seconds(150u64);
    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("alice0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // alice withdraws again at T=200, note this has same vested time as
    // first withdraw due to time_unit snap-back logic
    env.block.time = Timestamp::from_seconds(200u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    // bob withdraws again at T=200, note this has same vested time as
    // first withdraw due to time_unit snap-back logic
    env.block.time = Timestamp::from_seconds(200u64);
    let user_info = mock_info("bob0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();

    //
    let total_base_reward = 1_000_000u128;

    // for the first half of the reward distribution (T=100 -> T=150), both
    // alice and bob use the same boost value (50)
    let interval_base_reward = 500_000u128;
    let interval_boost_reward = 500_000u128;
    let alice_bonded = 100u128;
    let alice_boost_value = 50u128;
    let bob_bonded = 500u128;
    let bob_boost_value = 50u128;
    let total_bonded = alice_bonded + bob_bonded;
    let alice_boost_weight = (alice_bonded * alice_boost_value).integer_sqrt();
    let bob_boost_weight = (bob_bonded * bob_boost_value).integer_sqrt();
    let total_boost_weight = alice_boost_weight + bob_boost_weight;
    let alice_base_reward = Uint128::from(interval_base_reward) * Uint128::from(alice_bonded)
        / Uint128::from(total_bonded);
    let alice_boost_reward = Uint128::from(interval_boost_reward)
        * Decimal::from_ratio(alice_boost_weight, total_boost_weight);
    let mut alice_total_reward = alice_base_reward + alice_boost_reward;
    let bob_base_reward = Uint128::from(interval_boost_reward) * Uint128::from(bob_bonded)
        / Uint128::from(total_bonded);
    let bob_boost_reward = Uint128::from(interval_boost_reward)
        * Decimal::from_ratio(bob_boost_weight, total_boost_weight);
    let mut bob_total_reward = bob_base_reward + bob_boost_reward;
    let mut boost_index = Decimal::from_ratio(interval_base_reward, total_boost_weight);

    // for the second half of the reward distribution (T=150 -> T=200),
    // alice has a boost value of 75 but bob still has a boost value of 50 since
    // he hasn't re-activated his amps.  therefore, alice's percentage of the boost
    // reward goes up for this interval (from ~30 to ~35%).
    let alice_boost_value2 = 75u128;
    let alice_boost_weight2 = (alice_bonded * alice_boost_value2).integer_sqrt();
    let total_boost_weight2 = alice_boost_weight2 + bob_boost_weight;
    let alice_boost_reward2 = Uint128::from(interval_boost_reward)
        * Decimal::from_ratio(alice_boost_weight2, total_boost_weight2);
    let bob_boost_reward2 = Uint128::from(interval_boost_reward)
        * Decimal::from_ratio(bob_boost_weight, total_boost_weight2);
    alice_total_reward += alice_base_reward + alice_boost_reward2;
    bob_total_reward += bob_base_reward + bob_boost_reward2 + Uint128::from(1u128); // add 1 due to rounding
    boost_index = boost_index + Decimal::from_ratio(interval_base_reward, total_boost_weight2);

    // alice claims rewards after vesting period
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Prism,
    };
    env.block.time = Timestamp::from_seconds(vested_time1 + 1);
    let res = execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: alice_total_reward,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(alice_bonded),
            base_index: Decimal::from_ratio(total_base_reward, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index,
            boost_weight: Uint128::from(alice_boost_weight2),
            active_boost: Uint128::from(alice_boost_value2),
        }
    );

    // bob claims rewards after vesting period
    let user_info = mock_info("bob0000", &[]);
    let msg = ExecuteMsg::ClaimWithdrawnRewards {
        claim_type: ClaimType::Prism,
    };
    env.block.time = Timestamp::from_seconds(vested_time1 + 1);
    let res = execute(deps.as_mut(), env, user_info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "bob0000".to_string(),
                amount: bob_total_reward,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "bob0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(bob_bonded),
            base_index: Decimal::from_ratio(total_base_reward, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index,
            boost_weight: Uint128::from(bob_boost_weight),
            active_boost: Uint128::from(bob_boost_value),
        }
    );
}

#[test]
fn test_refresh_boost_unauthorized() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let mut boost_map = HashMap::new();
    boost_map.insert("alice0000".to_string(), Uint128::from(50u128));
    deps.querier.with_boost_querier(boost_map);

    // random contract can't call it
    let msg = ExecuteMsg::PrivilegedRefreshBoost {
        account: "alice0000".to_string(),
    };
    assert_eq!(
        execute(deps.as_mut(), mock_env(), mock_info("eve0000", &[]), msg).unwrap_err(),
        ContractError::Unauthorized {},
    );

    // boost contract can call it
    let msg = ExecuteMsg::PrivilegedRefreshBoost {
        account: "alice0000".to_string(),
    };
    assert_eq!(
        execute(deps.as_mut(), mock_env(), mock_info("boost0000", &[]), msg).unwrap(),
        Response::new().add_attributes(vec![
            ("action", "privileged_refresh_boost"),
            ("total_user_bonded", "0"),
            ("boost_amount", "50"),
        ]),
    );
}

#[test]
fn test_refresh_boost_authorized() {
    // Summary of test:
    //  T=0: - Set global boost to 50 AMPS.
    //  T=90 - Alice bonds 100 ylunas
    //  T=90 - Alice activates her boost (which is 50 AMPS)
    //  T=100 - Event starts
    //  T=110 - Alice withdraws her rewards
    //  T=110 - Set global boost to 0 AMPS.
    //  T=110 - Alice's boost becomes 0. xprism-boost contract calls PrivilegedRefreshBoost to notify us about that.
    //  T=120 - Alice withdraws her rewards. This time they are lower due to her lower boost.
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Set Alice boost value to 50 AMPS.
    let mut boost_map = HashMap::new();
    boost_map.insert("alice0000".to_string(), Uint128::from(50u128));
    deps.querier.with_boost_querier(boost_map.clone());

    // T=90: Alice bonds 100.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("ylunatoken0000", &[]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    execute(deps.as_mut(), env, info, msg).unwrap();

    // T=90: Alice activates boost immediately.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(90u64);
    let info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::ActivateBoost {};
    execute(deps.as_mut(), env, info, msg).unwrap();

    // T=100: Event starts.

    // T=110: alice withdraw rewards after 10 seconds, the entire boost pool is hers
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(110u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);
    // Verify her withdrawn rewards are as expected
    let total_base_reward1 = 100_000u128;
    let total_boost_reward1 = 100_000u128;
    let alice_bonded = 100u128;
    let alice_boost_value1 = 50u128;
    let total_bonded = alice_bonded;
    let alice_boost_weight1 = (alice_bonded * alice_boost_value1).integer_sqrt();
    let total_boost_weight1 = alice_boost_weight1;
    let alice_base_reward1 = Uint128::from(total_base_reward1) * Uint128::from(alice_bonded)
        / Uint128::from(total_bonded);
    let alice_boost_reward1 = Uint128::from(total_boost_reward1)
        * Decimal::from_ratio(alice_boost_weight1, total_boost_weight1);
    let alice_total_reward1 = alice_base_reward1 + alice_boost_reward1 - Uint128::from(1u128); // subtract 1 due to rounding
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![(vested_time, alice_total_reward1),],
            withdrawable: Uint128::zero(),
        }
    );
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(alice_bonded),
            base_index: Decimal::from_ratio(total_base_reward1, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::from_ratio(total_boost_reward1, total_boost_weight1),
            boost_weight: Uint128::from(alice_boost_weight1),
            active_boost: Uint128::from(alice_boost_value1),
        }
    );

    // T=110: Set Alice boost value to 0 AMPS and call refresh_boost.
    boost_map.insert("alice0000".to_string(), Uint128::from(0u128));
    deps.querier.with_boost_querier(boost_map);

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(110u64);
    let info = mock_info("boost0000", &[]);
    let msg = ExecuteMsg::PrivilegedRefreshBoost {
        account: "alice0000".to_string(),
    };
    assert_eq!(
        execute(deps.as_mut(), env.clone(), info, msg).unwrap(),
        Response::new().add_attributes(vec![
            ("action", "privileged_refresh_boost"),
            ("total_user_bonded", "100"),
            ("boost_amount", "0"),
        ]),
    );

    // T=120: alice withdraw rewards after another 10 seconds, but this time she gets no boost.
    env.block.time = Timestamp::from_seconds(120u64);
    let user_info = mock_info("alice0000", &[]);
    let msg = ExecuteMsg::WithdrawRewards {};
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let vested_time = compute_vested_time(env.block.time.seconds(), DEFAULT_VESTING_PERIOD);
    // Verify her withdrawn rewards are as expected. This time everything boost related should be zero.
    let total_base_reward2 = 100_000u128;
    let alice_base_reward2 = Uint128::from(total_base_reward2) * Uint128::from(alice_bonded)
        / Uint128::from(total_bonded);
    let alice_boost_reward2 = Uint128::zero();
    let alice_total_reward2 = alice_base_reward2 + alice_boost_reward2;
    assert_eq!(
        from_binary::<VestingStatusResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::VestingStatus {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        VestingStatusResponse {
            scheduled_vests: vec![(vested_time, alice_total_reward1 + alice_total_reward2),],
            withdrawable: Uint128::zero(),
        }
    );
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::from(alice_bonded),
            base_index: Decimal::from_ratio(total_base_reward1, total_bonded)
                + Decimal::from_ratio(total_base_reward2, total_bonded),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::from_ratio(total_boost_reward1, total_boost_weight1),
            boost_weight: Uint128::zero(),
            active_boost: Uint128::zero(),
        }
    );
}

#[test]
fn test_activate_boost_for_user_without_anything_bonded() {
    // Test what happens when a user who has never bonded anything calls activate_boost.
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        operator: "op0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
        gov: "gov0000".to_string(),
        base_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
        boost_distribution_schedule: (100u64, 200u64, Uint128::from(1_000_000u128)),
        boost_contract: "boost0000".to_string(),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
        vesting_period: DEFAULT_VESTING_PERIOD,
        min_bond_amount: Uint128::from(100u128),
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut boost_map = HashMap::new();
    boost_map.insert("alice0000".to_string(), Uint128::from(50u128));
    deps.querier.with_boost_querier(boost_map);

    let msg = ExecuteMsg::ActivateBoost {};
    let info = mock_info("alice0000", &[]);
    assert_eq!(
        execute(deps.as_mut(), mock_env(), info, msg).unwrap(),
        Response::new().add_attributes(vec![
            ("action", "activate_boost"),
            ("total_user_bonded", "0"),
            ("boost_amount", "50"),
        ]),
    );
    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "alice0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            bond_amount: Uint128::zero(),
            base_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            boost_index: Decimal::zero(),
            boost_weight: Uint128::zero(),
            active_boost: Uint128::from(50_u128),
        }
    );
}
