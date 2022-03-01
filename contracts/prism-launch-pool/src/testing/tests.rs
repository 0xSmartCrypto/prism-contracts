use crate::state::{RewardInfo, BOND_AMOUNTS};
use crate::{
    contract::{execute, instantiate, query},
    error::ContractError,
    state::{BOND_AMOUNTS, REWARD_INFO},
};
use cosmwasm_std::attr;
use cosmwasm_std::OwnedDeps;
use cosmwasm_std::{
    from_binary,
    testing::{mock_env, mock_info},
    to_binary, Addr, CosmosMsg, Decimal, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::{Asset, AssetInfo};
use prism_common::testing::mock_querier::{mock_dependencies, MOCK_CONTRACT_ADDR};
use prism_protocol::launch_pool::{
    ConfigResponse, Cw20HookMsg, DistributionStatusResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    RewardInfoResponse, VestingStatusResponse,
};
use prism_protocol::yasset_staking::{
    Cw20HookMsg as StakingHookMsg, ExecuteMsg as StakingExecuteMsg,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    // invalid distribution schedule
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 99u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidDistributionSchedule {});

    // successful init
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            prism_token: "prism0000".to_string(),
            distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            yluna_staking: "ylunastaking0000".to_string(),
            yluna_token: "ylunatoken0000".to_string(),
        }
    );
}

#[test]
fn withdraw_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
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
                (1814400u64, Uint128::from(500000u128)) // 1000000 / 2
            ],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(500000u128),
            total_bond_amount: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(500000u128, 100u128),
        }
    );

    // withdraw rewards after 500 seconds (farming ended after 100 sec)
    env.block.time = Timestamp::from_seconds(600u64);

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
            scheduled_vests: vec![(1814400u64, Uint128::from(1000000u128))],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(1000000u128),
            total_bond_amount: Uint128::from(100u128),
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
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
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
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(500000u128),
            total_bond_amount: Uint128::zero(),
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
            scheduled_vests: vec![(1814400u64, Uint128::from(500000u128))],
            withdrawable: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(500000u128),
            total_bond_amount: Uint128::from(100u128),
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
            index: Decimal::from_ratio(500000u128, 100u128),
            pending_reward: Uint128::zero(),
        }
    );
}

/// Test that withdraw_rewards_bulk returns error if called by non-owner.
#[test]
fn withdraw_rewards_bulk_auth() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::WithdrawRewardsBulk {
        limit: 2,
        start_after_address: Some(String::from("monkey")),
    };

    // Non-owner fails.
    let user_info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), user_info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Owner succeeds.
    let user_info = mock_info("owner0000", &[]);
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
        prism_token: "prism0000".to_string(),
        // Distribute 100 PRISMs between time 10s to 20s.
        distribution_schedule: (10u64, 20u64, Uint128::from(100u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
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
    let user_info = mock_info("owner0000", &[]);
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
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(100u128),
            total_bond_amount: Uint128::from(10u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(100u128, 10u128),
        }
    );

    // Still time 21: Second call to withdraw_rewards_bulk.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(21u64); // After event ends.
    let user_info = mock_info("owner0000", &[]);
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
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(100u128),
            total_bond_amount: Uint128::from(10u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(100u128, 10u128),
        }
    );

    // Still time 21: Last call to cover Alice.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(21u64);
    let user_info = mock_info("owner0000", &[]);
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
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(100u128),
            total_bond_amount: Uint128::from(10u128),
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
        prism_token: "prism0000".to_string(),
        distribution_schedule: (
            100000000000000u64,
            200000000000000u64,
            Uint128::from(1000000u128),
        ),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // wrong token
    let info = mock_info("lp00001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // correct token
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
            index: Decimal::zero(),
            pending_reward: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatusResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::zero(),
            total_bond_amount: Uint128::from(100u128),
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
        prism_token: "prism0000".to_string(),
        // Distribute 5000 reward tokens during a 30-second event.
        distribution_schedule: (30u64, 60u64, Uint128::from(5_000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
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
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(5_000u128),
            total_bond_amount: Uint128::from(75u128),
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
        .unwrap(),
        DistributionStatusResponse {
            total_distributed: Uint128::from(5_000u128),
            total_bond_amount: Uint128::zero(),
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
            index: Decimal::from_ratio(5_000u128, 100u128),
            pending_reward: Uint128::from(5_000u128),
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
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
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

    // try to claim before claim end_time expires
    let msg = ExecuteMsg::ClaimWithdrawnRewards {};
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
                (1814400u64, Uint128::from(500000u128)) // 1000000 / 2
            ],
            withdrawable: Uint128::zero(),
        }
    );

    env.block.time = Timestamp::from_seconds(1814401u64);

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
                (1814400u64, Uint128::from(500000u128)) // 1000000 / 2
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
        prism_token: "prism0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
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
        |time: u64,
         deps: &OwnedDeps<_, _, _>,
         want_distribution_status: DistributionStatusResponse| {
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(time);
            assert_eq!(
                from_binary::<DistributionStatusResponse>(
                    &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
                )
                .unwrap(),
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
        prism_token: "prism0000".to_string(),
        // Distribute 100k PRISMs between time 100s to 200s.
        distribution_schedule: (100u64, 200u64, Uint128::from(100_000u128)),
        yluna_staking: "ylunastaking0000".to_string(),
        yluna_token: "ylunatoken0000".to_string(),
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Time 110: No one has bonded anything yet. Rewards emitted since time 100
    // get accumulated for a lucky future winner in the pending_reward field.
    check_global_distribution_status(
        110,
        &deps,
        DistributionStatusResponse {
            total_distributed: Uint128::from(10_000u128),
            total_bond_amount: Uint128::from(0u128),
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(10_000u128),
            total_bond_amount: Uint128::from(2u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128),
        },
    );
    check_individual_reward_info(
        110,
        &deps,
        "bob",
        RewardInfo {
            index: Decimal::zero(), // this index == 0 is what will allow Bob to claim the initial rewards. Notice that Alice's index field below is != 0.
            pending_reward: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(110, &deps, "bob", vec![]);
    bond(110, &mut deps, "alice", 3);
    check_bond_balances(&deps, vec![("alice", 3), ("bob", 2)]);
    check_global_distribution_status(
        110,
        &deps,
        DistributionStatusResponse {
            total_distributed: Uint128::from(10_000u128),
            total_bond_amount: Uint128::from(5u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(10_000u128, 2u128),
        },
    );
    check_individual_reward_info(
        110,
        &deps,
        "alice",
        RewardInfo {
            index: Decimal::from_ratio(10_000u128, 2u128),
            pending_reward: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(110, &deps, "alice", vec![]);

    // Time 120: Alice unbonds part of her position.
    unbond(120, &mut deps, "alice", 1);
    check_bond_balances(&deps, vec![("alice", 2), ("bob", 2)]);
    check_global_distribution_status(
        120,
        &deps,
        DistributionStatusResponse {
            total_distributed: Uint128::from(20_000u128),
            total_bond_amount: Uint128::from(4u128),
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
            index: Decimal::from_ratio(10_000u128, 2u128) + Decimal::from_ratio(10_000u128, 5u128),
            pending_reward: Uint128::from(10_000 / 5 * 3_u128),
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(30_000u128),
            total_bond_amount: Uint128::from(4u128),
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
            index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128),
            pending_reward: Uint128::zero(),
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(40_000u128),
            total_bond_amount: Uint128::from(8u128),
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
            index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128),
            pending_reward: Uint128::zero(),
        },
    );
    check_individual_vesting_schedule(140, &deps, "carol", vec![]);

    // Time 150: Rewards for Alice are withdrawn by the bot.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(150);
    let info = mock_info("owner0000", &[]);
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(50_000u128),
            total_bond_amount: Uint128::from(8u128),
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
            index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128),
            pending_reward: Uint128::zero(),
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(60_000u128),
            total_bond_amount: Uint128::from(50u128),
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
            index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128),
            pending_reward: Uint128::from(10_000 / 8 * 4 + 10_000 / 8 * 4_u128),
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(100_000u128),
            total_bond_amount: Uint128::from(4u128),
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
            index: Decimal::from_ratio(10_000u128, 2u128)
                + Decimal::from_ratio(10_000u128, 5u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 4u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(10_000u128, 8u128)
                + Decimal::from_ratio(40_000u128, 50u128),
            pending_reward: Uint128::from(10_000 / 8 * 4 + 10_000 / 8 * 4 + 40_000 / 50 * 46_u128),
        },
    );
    check_individual_vesting_schedule(210, &deps, "carol", vec![]);

    // Time 300: Rewards for all users are withdrawn by the bot.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(300);
    let info = mock_info("owner0000", &[]);
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
        DistributionStatusResponse {
            total_distributed: Uint128::from(100_000u128),
            total_bond_amount: Uint128::from(4u128),
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
                index: Decimal::from_ratio(10_000u128, 2u128)
                    + Decimal::from_ratio(10_000u128, 5u128)
                    + Decimal::from_ratio(10_000u128, 4u128)
                    + Decimal::from_ratio(10_000u128, 4u128)
                    + Decimal::from_ratio(10_000u128, 8u128)
                    + Decimal::from_ratio(10_000u128, 8u128)
                    + Decimal::from_ratio(40_000u128, 50u128),
                pending_reward: Uint128::zero(),
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
