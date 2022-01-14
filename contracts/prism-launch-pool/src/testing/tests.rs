use crate::{
    contract::{execute, instantiate, query},
    error::ContractError,
};
use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{
    from_binary,
    testing::{mock_env, mock_info},
    to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
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

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
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
            reward_distribution: "rewarddistribution0000".to_string(),
            distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
            yasset_staking: "yassetstaking0000".to_string(),
            yasset_token: "yassettoken0000".to_string(),
        }
    );
}

#[test]
fn withdraw_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
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
    let info = mock_info("yassettoken0000", &[]);
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
            &query(deps.as_ref(), env.clone(), QueryMsg::DistributionStatus {},).unwrap(),
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
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
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
    let info = mock_info("yassettoken0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

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

#[test]
fn bond() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (
            100000000000000u64,
            200000000000000u64,
            Uint128::from(1000000u128),
        ),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
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
    let info = mock_info("yassettoken0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yassettoken0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "yassetstaking0000".to_string(),
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
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (
            100000000000000u64,
            200000000000000u64,
            Uint128::from(1000000u128),
        ),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("yassettoken0000", &[]);
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // try to unbond from a different sender with nothing bonded
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(101u128)),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
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
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        res,
        ContractError::InvalidUnbond {
            reason: "can not unbond more than the bonded amount".to_string()
        }
    );

    // successful unbond of 25
    let unbond_amt = Uint128::from(25u128);
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(unbond_amt),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yassetstaking0000".to_string(),
                msg: to_binary(&StakingExecuteMsg::Unbond {
                    amount: Some(unbond_amt),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yassettoken0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0000".to_string(),
                    amount: unbond_amt,
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    // successful unbond of remaining 75 (using None as amount)
    let remaining_amt = Uint128::from(75u128);
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond { amount: None };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yassetstaking0000".to_string(),
                msg: to_binary(&StakingExecuteMsg::Unbond {
                    amount: Some(remaining_amt),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yassettoken0000".to_string(),
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
                mock_env(),
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

    assert_eq!(
        from_binary::<RewardInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::RewardInfo {
                    staker_addr: "addr0000".to_string()
                }
            )
            .unwrap(),
        )
        .unwrap(),
        RewardInfoResponse {
            index: Decimal::zero(),
            pending_reward: Uint128::zero(),
        }
    );
}

#[test]
fn claim_withdrawn_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
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
    let info = mock_info("yassettoken0000", &[]);
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
}

#[test]
fn admin_withdraw_rewards() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        reward_distribution: "rewarddistribution0000".to_string(),
        distribution_schedule: (100u64, 200u64, Uint128::from(1000000u128)),
        yasset_staking: "yassetstaking0000".to_string(),
        yasset_token: "yassettoken0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::AdminWithdrawRewards {};

    // wrong adddress attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(100u128),
        },
    )]);
    deps.querier.with_token_balances(&[(
        &"mir0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(200u128))],
    )]);

    // correct address
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yassetstaking0000".to_string(),
                msg: to_binary(&StakingExecuteMsg::ClaimRewards {}).unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::AdminSendWithdrawnRewards {
                    original_balances: vec![
                        Asset {
                            info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            amount: Uint128::from(100u128),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("mir0000"),
                            },
                            amount: Uint128::from(200u128),
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
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("mir0000"),
                },
                amount: Uint128::from(200u128),
            },
        ],
    };

    // simulate that the contract received rewards after claiming
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(250u128),
        },
    )]);
    deps.querier.with_token_balances(&[(
        &"mir0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(400u128))],
    )]);

    // wrong adddress attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // correct address
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "owner0000".to_string(),
                amount: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::from(150u128)
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mir0000".to_string(),
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
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: Uint128::from(250u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("mir0000"),
                },
                amount: Uint128::from(400u128),
            },
        ],
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages, vec![])
}
