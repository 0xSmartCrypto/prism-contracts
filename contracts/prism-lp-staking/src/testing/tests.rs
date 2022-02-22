use crate::contract::{execute, instantiate, query};
use crate::ContractError;
use cosmwasm_std::testing::{mock_env, mock_info, MockApi};
use cosmwasm_std::{
    attr, from_binary, to_binary, CosmosMsg, Decimal, MemoryStorage, OwnedDeps, StdError, SubMsg,
    Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use prism_testing::mock_querier::{mock_dependencies, WasmMockQuerier, MOCK_CONTRACT_ADDR};
use prism_protocol::lp_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponseItem, StakerInfoResponse, StakersInfoResponse, UnbondOrdersResponse,
};

// helper to successfully init with reasonable defaults
pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: default_distribution_schedule(),
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100u64),
            ("lp00002".to_string(), 20u64, 100u64),
        ],
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

pub fn default_distribution_schedule() -> Vec<(u64, u64, Uint128)> {
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();
    return vec![
        (
            default_genesis_seconds,
            default_genesis_seconds + 100,
            Uint128::from(1000000u128),
        ),
        (
            default_genesis_seconds + 100,
            default_genesis_seconds + 200,
            Uint128::from(10000000u128),
        ),
    ];
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();
    let info = mock_info("addr0000", &[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100u64),
            ("lp00002".to_string(), 20u64, 100u64),
            ("lp00001".to_string(), 7u64, 100u64),
        ],
    };
    let err = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicateStakingToken {});

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![(200, 100, Uint128::from(1000000u128))],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100u64),
            ("lp00002".to_string(), 20u64, 100u64),
        ],
    };
    let err = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidDistributionSchedule {});

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100u64),
            ("lp00002".to_string(), 20u64, 100u64),
        ],
    };

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            prism_token: "prism0000".to_string(),
            distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
            total_weight: 30u64,
        }
    );

    // query the created pools
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            staking_token: "lp00001".to_string(),
        },
    )
    .unwrap();
    let pool: PoolInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        pool,
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            last_distributed: default_genesis_seconds,
            total_bond_amount: Uint128::zero(),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            unbond_period: 100u64,
        }
    );
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            staking_token: "lp00002".to_string(),
        },
    )
    .unwrap();
    let pool: PoolInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        pool,
        PoolInfoResponse {
            weight: 20u64,
            staking_token: "lp00002".to_string(),
            pending_reward: Uint128::zero(),
            last_distributed: default_genesis_seconds,
            total_bond_amount: Uint128::zero(),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            unbond_period: 100u64,
        }
    );
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("lp00001", &[]);
    let mut env = mock_env();
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // same as above, but pass in staking token instead of None
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: Some("lp00001".to_string()),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                }
            )
            .unwrap()
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::from(100u128),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            last_distributed: default_genesis_seconds,
            unbond_period: 100u64,
        }
    );

    // bond 100 more tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    env.block.time = env.block.time.plus_seconds(10);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(33333u128), // 100000 * 10 / (10 + 20)
                bond_amount: Uint128::from(200u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                }
            )
            .unwrap()
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::from(200u128),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::from_ratio(33333u128, 100u128),
            last_distributed: default_genesis_seconds + 10,
            unbond_period: 100u64,
        }
    );

    // failed if stake a different token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("staking0001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {})
}

#[test]
fn test_auto_stake_hook() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();
    init(&mut deps);

    // bond normally
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                }
            )
            .unwrap()
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::from(100u128),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            last_distributed: default_genesis_seconds,
            unbond_period: 100u64,
        }
    );

    deps.querier.with_token_balances(&[(
        &"lp00001".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(250u128))],
    )]); // 100 bonded + new 150

    // auto stake hook - invalid staking token
    let msg = ExecuteMsg::AutoStakeHook {
        staking_token: "lp00003".to_string(),
    };
    let info = mock_info("addr0001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {});

    // now bond with auto stake book
    let msg = ExecuteMsg::AutoStakeHook {
        staking_token: "lp00001".to_string(),
    };

    let info = mock_info("addr0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staking_token", "lp00001"),
            attr("staker", "addr0001"),
            attr("amount", "150"),
        ]
    );

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0001".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0001".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::from(150u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                }
            )
            .unwrap()
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::from(250u128),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            last_distributed: default_genesis_seconds,
            unbond_period: 100u64,
        }
    );
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // unbond 50 tokens; an order is created
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(50u128)),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 0);
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unbond"),
            attr("staking_token", "lp00001"),
            attr("staker", "addr0000"),
            attr("amount", "50"),
            attr("unbond_order_created", "true"),
            attr(
                "expected_expire_time",
                mock_env()
                    .block
                    .time
                    .plus_seconds(100)
                    .seconds()
                    .to_string()
            ),
        ]
    );

    // bond amount should decrease
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: Some("lp00001".to_string()),
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::from(50u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // pending withdraw amount should increase
    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                }
            )
            .unwrap()
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::from(50u128),
            total_pending_withdraw: Uint128::from(50u128),
            reward_index: Decimal::zero(),
            last_distributed: mock_env().block.time.seconds(),
            unbond_period: 100u64,
        }
    );

    // query unbond orders
    assert_eq!(
        from_binary::<UnbondOrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::UnbondOrders {
                    staker: "addr0000".to_string(),
                    staking_token: "lp00001".to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        UnbondOrdersResponse {
            withdrawable_amount: Uint128::zero(),
            orders: vec![(
                mock_env().block.time.plus_seconds(100).seconds(),
                Uint128::from(50u128)
            )]
        }
    );

    // try to claim unbonded right after, should fail
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };

    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::NothingAvailableToWithdraw {});

    // wait 101 seconds
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(101);

    // query unbond orders
    assert_eq!(
        from_binary::<UnbondOrdersResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::UnbondOrders {
                    staker: "addr0000".to_string(),
                    staking_token: "lp00001".to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        UnbondOrdersResponse {
            withdrawable_amount: Uint128::from(50u64), // now its withdrawable
            orders: vec![(
                mock_env().block.time.plus_seconds(100).seconds(),
                Uint128::from(50u128)
            )]
        }
    );

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(50u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // now there should be no orders
    assert_eq!(
        from_binary::<UnbondOrdersResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::UnbondOrders {
                    staker: "addr0000".to_string(),
                    staking_token: "lp00001".to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        UnbondOrdersResponse {
            withdrawable_amount: Uint128::zero(),
            orders: vec![]
        }
    );

    // pending withdraw should be zero
    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                }
            )
            .unwrap()
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 10u64,
            staking_token: "lp00001".to_string(),
            pending_reward: Uint128::zero(),
            total_bond_amount: Uint128::from(50u128),
            total_pending_withdraw: Uint128::zero(), // back to zero
            reward_index: Decimal::zero(),
            last_distributed: 1571797419, // current time minus 101
            unbond_period: 100u64,
        }
    );

    // try to unbond too much
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(70u128)),
    };

    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidUnbondAmount {});

    // unbond remaining
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: None,
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another withdraw order is created
    assert_eq!(
        from_binary::<UnbondOrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::UnbondOrders {
                    staker: "addr0000".to_string(),
                    staking_token: "lp00001".to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        UnbondOrdersResponse {
            withdrawable_amount: Uint128::zero(),
            orders: vec![(
                mock_env().block.time.plus_seconds(100).seconds(),
                Uint128::from(50u128)
            )]
        }
    );
}

#[test]
fn test_unbond_2() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![
            (
                default_genesis_seconds,
                default_genesis_seconds + 300,
                Uint128::from(1000000u128),
            ),
            (
                default_genesis_seconds + 300,
                default_genesis_seconds + 400,
                Uint128::from(10000000u128),
            ),
        ],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100u64),
            ("lp00002".to_string(), 20u64, 100u64),
        ],
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut env = mock_env();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // unbond 10
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(10u128)),
    };
    let info = mock_info("addr0000", &[]);
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // wait 10 seconds and unbond 20
    env.block.time = env.block.time.plus_seconds(10);

    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(20u128)),
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // wait 10 seconds, nothing withdrawable
    env.block.time = env.block.time.plus_seconds(10);

    // claim unbonded tokens; failed because they are locked for 100 seconds
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::NothingAvailableToWithdraw {});

    // wait 81 seconds, 10 unlocks
    env.block.time = env.block.time.plus_seconds(81);

    // normal claim unbond, tokens are unlocked
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(10u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // wait 10 seconds, 20 unlocks
    env.block.time = env.block.time.plus_seconds(10);

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(20u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_compute_reward() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    let mut env = mock_env();
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // 10 seconds passed
    // 100,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(10);

    // bond 100 more tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(33333u128), // 100000 * 10 / (10 + 20)
                bond_amount: Uint128::from(200u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // 100 seconds passed (90 first slot + 10 next slot)
    // 900,000 + 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    // unbond
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(666665u128), // 33333 + 1900000 * 10 / (10 + 20)
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    // invalid claim - invalid staking token
    let msg = ExecuteMsg::ClaimRewards {
        staking_token: Some("lp00003".to_string()),
    };
    let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {});

    // valid claim
    let msg = ExecuteMsg::ClaimRewards {
        staking_token: Some("lp00001".to_string()),
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(333333u128), // 1,000,000 * 10 / (10 + 20)
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_claim_all_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // bond 100 LP1 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // bond 100 LP2 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00002", &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    // claim with None
    let msg = ExecuteMsg::ClaimRewards {
        staking_token: None,
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                // 1,000,000 * 10 / (10 + 20) + 1,000,000 * 20 / (10 + 20)
                // = 333333 + 666666 (not the full 1M due to rounding down)
                amount: Uint128::from(999999u128),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );
}

#[test]
fn test_query_stakers() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("lp00001", &[]);
    let env = mock_env();
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(200u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0002".to_string(),
        amount: Uint128::from(300u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::TokenStakersInfo {
                    staking_token: "lp00001".to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakersInfoResponse {
            stakers: vec![
                StakerInfoResponse {
                    staker: "addr0000".to_string(),
                    reward_infos: vec![RewardInfoResponseItem {
                        staking_token: "lp00001".to_string(),
                        pending_reward: Uint128::zero(),
                        bond_amount: Uint128::from(100u128),
                        withdrawable_amount: Uint128::zero(),
                    }]
                },
                StakerInfoResponse {
                    staker: "addr0001".to_string(),
                    reward_infos: vec![RewardInfoResponseItem {
                        staking_token: "lp00001".to_string(),
                        pending_reward: Uint128::zero(),
                        bond_amount: Uint128::from(200u128),
                        withdrawable_amount: Uint128::zero(),
                    }]
                },
                StakerInfoResponse {
                    staker: "addr0002".to_string(),
                    reward_infos: vec![RewardInfoResponseItem {
                        staking_token: "lp00001".to_string(),
                        pending_reward: Uint128::zero(),
                        bond_amount: Uint128::from(300u128),
                        withdrawable_amount: Uint128::zero(),
                    }]
                }
            ]
        }
    );
    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::TokenStakersInfo {
                    staking_token: "lp00001".to_string(),
                    start_after: Some("addr0000".to_string()),
                    limit: Some(1u32),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakersInfoResponse {
            stakers: vec![StakerInfoResponse {
                staker: "addr0001".to_string(),
                reward_infos: vec![RewardInfoResponseItem {
                    staking_token: "lp00001".to_string(),
                    pending_reward: Uint128::zero(),
                    bond_amount: Uint128::from(200u128),
                    withdrawable_amount: Uint128::zero(),
                }]
            },]
        }
    );
}

#[test]
fn test_update_owner() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::UpdateOwner {
        owner: "newowner0000".to_string(),
    };

    // unauthorized
    let info = mock_info("addr0001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // success
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // query config, verify new owner
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(config.owner, "newowner0000");

    // previous owner unauthorized
    let info = mock_info("owner0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_add_distribution_schedule() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let schedule_to_add = vec![
        (
            default_genesis_seconds + 200,
            default_genesis_seconds + 300,
            Uint128::from(1000000u128),
        ),
        (
            default_genesis_seconds + 300,
            default_genesis_seconds + 400,
            Uint128::from(10000000u128),
        ),
    ];

    // unauthorized
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::AddDistributionSchedule {
        schedule: schedule_to_add.clone(),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // invalid schedule - start time < current time
    let mut schedule = schedule_to_add.clone();
    schedule[0].0 = default_genesis_seconds - 1;
    let msg = ExecuteMsg::AddDistributionSchedule { schedule };
    let info = mock_info("owner0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidDistributionSchedule {});

    // invalid schedule - end time == start time
    let mut schedule = schedule_to_add.clone();
    schedule[0].1 = schedule[0].0;
    let msg = ExecuteMsg::AddDistributionSchedule { schedule };
    let info = mock_info("owner0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidDistributionSchedule {});

    // success
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::AddDistributionSchedule {
        schedule: schedule_to_add.clone(),
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // query config, verify new schedule
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    let mut new_schedule = default_distribution_schedule();
    new_schedule.extend(schedule_to_add);
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            prism_token: "prism0000".to_string(),
            distribution_schedule: new_schedule,
            total_weight: 30u64,
        }
    );
}

#[test]
fn test_overlapping_distribution_schedule() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let mut env = mock_env();
    let default_genesis_seconds: u64 = env.block.time.seconds();

    let schedule_to_add = vec![(
        default_genesis_seconds + 50,
        default_genesis_seconds + 150,
        Uint128::from(1000000u128),
    )];

    // success
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::AddDistributionSchedule {
        schedule: schedule_to_add.clone(),
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // query config, verify new schedule
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    let mut new_schedule = default_distribution_schedule();
    new_schedule.extend(schedule_to_add);
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            prism_token: "prism0000".to_string(),
            distribution_schedule: new_schedule,
            total_weight: 30u64,
        }
    );

    // addr0000 bonds 100 lp00001 at t=0
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // query pool rewards at t=125
    // we get 1/3 of the rewards the whole time
    // schedule 1 - 1/3 * 1M = 333333
    // schedule 2 - 1/3 * 1/4 * 10M = 833333
    // schedule 3 - 1/3 * 3/4 * 1M = 250000 (rounded down to 249999)
    env.block.time = env.block.time.plus_seconds(125);
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(1416665u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );
}

#[test]
fn test_register_staking_token() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::RegisterStakingToken {
        staking_token: "lp00003".to_string(),
        unbond_period: 100u64,
        weight: 30u64,
    };

    // unauthorized
    let info = mock_info("addr0001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // already exists
    let info = mock_info("owner0000", &[]);
    let invalid_msg = ExecuteMsg::RegisterStakingToken {
        staking_token: "lp00001".to_string(),
        unbond_period: 100u64,
        weight: 30u64,
    };
    let err = execute(deps.as_mut(), mock_env(), info, invalid_msg).unwrap_err();
    assert_eq!(err, ContractError::AlreadyExists {});

    // query pool info for lp00003, will fail, doesn't exist yet
    assert_eq!(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                staking_token: "lp00003".to_string()
            },
        )
        .unwrap_err(),
        ContractError::Std(StdError::NotFound {
            kind: "prism_lp_staking::state::PoolInfo".to_string()
        })
    );

    // successful register of lp00003
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages, vec![]);
    assert_eq!(
        res.attributes,
        vec![attr("action", "register_staking_token")]
    );

    // query pool info for lp00003 again, will now succeed
    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::PoolInfo {
                    staking_token: "lp00003".to_string()
                },
            )
            .unwrap(),
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 30u64,
            last_distributed: mock_env().block.time.seconds(),
            staking_token: "lp00003".to_string(),
            total_bond_amount: Uint128::zero(),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            unbond_period: 100u64,
        }
    );
}

#[test]
fn test_register_staking_token_with_bonding() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    let mut env = mock_env();

    // addr0000 bonds 100 lp00001 at t=0
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // register lp00003 with weight=30 at t=50
    env.block.time = env.block.time.plus_seconds(50);
    let msg = ExecuteMsg::RegisterStakingToken {
        staking_token: "lp00003".to_string(),
        unbond_period: 100u64,
        weight: 30u64,
    };
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // verify config total weight updated to 60
    let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(config.total_weight, 60u64);

    // query pool rewards at end of 1st distribution schedule
    // our portion of the 1M in rewards change from 1/3 to 1/6 at t=50 due to the
    // newly registered staking token (lp00003) which has a weight of 30.
    // rewards should be broken into two segments (1/3 * 500K) + (1/6 * 500K) = 249999
    env.block.time = env.block.time.plus_seconds(50);
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(249999u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );
}

#[test]
fn test_update_staking_token() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::UpdateStakingToken {
        staking_token: "lp00001".to_string(),
        unbond_period: Some(101u64),
        weight: Some(20u64),
    };

    // unauthorized
    let info = mock_info("addr0001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // invalid staking token
    let info = mock_info("owner0000", &[]);
    let invalid_msg = ExecuteMsg::UpdateStakingToken {
        staking_token: "lp00003".to_string(),
        unbond_period: Some(101u64),
        weight: Some(20u64),
    };
    let err = execute(deps.as_mut(), mock_env(), info, invalid_msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {});

    // invalid staking token with no weight (different path)
    let info = mock_info("owner0000", &[]);
    let invalid_msg = ExecuteMsg::UpdateStakingToken {
        staking_token: "lp00003".to_string(),
        unbond_period: Some(101u64),
        weight: None,
    };
    let err = execute(deps.as_mut(), mock_env(), info, invalid_msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {});

    // successful update  of lp00001
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages, vec![]);
    assert_eq!(res.attributes, vec![attr("action", "update_staking_token")]);

    // query pool info for lp00001, verify params updated
    assert_eq!(
        from_binary::<PoolInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::PoolInfo {
                    staking_token: "lp00001".to_string()
                },
            )
            .unwrap(),
        )
        .unwrap(),
        PoolInfoResponse {
            weight: 20u64,
            last_distributed: mock_env().block.time.seconds(),
            staking_token: "lp00001".to_string(),
            total_bond_amount: Uint128::zero(),
            total_pending_withdraw: Uint128::zero(),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            unbond_period: 101u64,
        }
    );
}

#[test]
fn test_update_staking_token_with_bonding_increase_weight() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    let mut env = mock_env();

    // addr0000 bonds 100 lp00001 at t=0
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // update staking token lp00001 with weight=20 at t=50
    env.block.time = env.block.time.plus_seconds(50);
    let msg = ExecuteMsg::UpdateStakingToken {
        staking_token: "lp00001".to_string(),
        unbond_period: Some(100u64),
        weight: Some(20u64),
    };
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // verify config total weight updated to 40
    let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(config.total_weight, 40u64);

    // query pool rewards at end of 1st distribution schedule
    // our portion of the 1M in rewards change from 1/3 to 1/2 at t=50 due to the
    // updated weight of lp00001 from 10 to 20.
    // rewards should be broken into two segments (1/3 * 500K) + (1/2 * 500K) = 416666
    env.block.time = env.block.time.plus_seconds(50);
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(416666u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );
}

#[test]
fn test_update_staking_token_with_bonding_decrease_weight() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    let mut env = mock_env();

    // addr0000 bonds 100 lp00001 at t=0
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // update staking token lp00001 with weight=5 at t=50
    env.block.time = env.block.time.plus_seconds(50);
    let msg = ExecuteMsg::UpdateStakingToken {
        staking_token: "lp00001".to_string(),
        unbond_period: Some(100u64),
        weight: Some(5u64),
    };
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // verify config total weight updated to 25
    let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(config.total_weight, 25u64);

    // query pool rewards at end of 1st distribution schedule
    // our portion of the 1M in rewards change from 1/3 to 1/5 at t=50 due to the
    // updated weight of lp00001 from 10 to 5.
    // rewards should be broken into two segments (1/3 * 500K) + (1/5 * 500K) = 216666
    env.block.time = env.block.time.plus_seconds(50);
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(266666u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );
}

/// a few more unbond tests checking for error conditions and verifying
/// proper reward claiming and storage cleanup after unbonding
#[test]
fn test_unbond_3() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: default_distribution_schedule(),
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 0u64),
            ("lp00002".to_string(), 20u64, 0u64),
        ],
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // invalid unbond - invalid staking token
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00003".to_string(),
        amount: Some(Uint128::from(60u128)),
    };

    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {});

    // invalid unbond - nothing staked
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00002".to_string(),
        amount: Some(Uint128::from(60u128)),
    };

    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::NothingStaked {});

    // check staker info
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(0u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // valid unbond. Since there is no unbond period, the tokens are sent directly
    let info = mock_info("addr0000", &[]);
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(100); // accrue some rewards
                                                       // normal unbond
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // test staker info after unbonding everything
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(333333u128), // 1000000 * 10 / (10 + 20)
                bond_amount: Uint128::from(0u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // test claim rewards after unbonding everything
    let msg = ExecuteMsg::ClaimRewards {
        staking_token: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(333333u128),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    // verify rewards empty
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![]
        }
    );
}

#[test]
fn test_claim_unbonded() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: default_distribution_schedule(),
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100u64),
            ("lp00002".to_string(), 20u64, 100u64),
        ],
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // invalid claim - invalid staking token
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00003".to_string(),
    };
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidStakingToken {});

    // invalid claim - nothing staked
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00002".to_string(),
    };
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::NothingStaked {});

    // check staker info
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(0u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // valid unbond at T=0
    let info = mock_info("addr0000", &[]);
    let mut env = mock_env();
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 0);
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unbond"),
            attr("staking_token", "lp00001"),
            attr("staker", "addr0000"),
            attr("amount", "100"),
            attr("unbond_order_created", "true"),
            attr(
                "expected_expire_time",
                mock_env()
                    .block
                    .time
                    .plus_seconds(100)
                    .seconds()
                    .to_string()
            ),
        ]
    );

    // failure - unbond again, nothing available
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::NothingAvailableToUnbond {});

    // claim unbonded at T=101
    env.block.time = env.block.time.plus_seconds(101);
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    // verify rewards empty
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![]
        }
    );
}

#[test]
fn test_claim_unbonded_2() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: default_distribution_schedule(),
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 0u64),
            ("lp00002".to_string(), 20u64, 0u64),
        ],
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // check staker info
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(0u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // valid unbond at T=0
    let info = mock_info("addr0000", &[]);
    let env = mock_env();
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // failure - unbond again, nothing staked
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::NothingStaked {});

    // verify rewards empty
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![]
        }
    );
}

#[test]
fn test_max_withdraws_per_tx() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: default_distribution_schedule(),
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 100000u64),
            ("lp00002".to_string(), 20u64, 100000u64),
        ],
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens, 75 times, at 100 second intervals (total of 7500)
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    let mut env = mock_env();
    for _ in 0..75 {
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        env.block.time = env.block.time.plus_seconds(100);
    }

    // check staker info, we have 10K bonded amount and 3.66M in rewards
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(3666666u128), // 1M / 3 + 10M / 3
                bond_amount: Uint128::from(7500u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // unbond 100 tokens, 75 times, at 100 second intervals
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let info = mock_info("addr0000", &[]);
    for _ in 0..75 {
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        env.block.time = env.block.time.plus_seconds(100);
    }

    // check staker info, we have 0 bonded amount and 3.66M in rewards
    let staker_response = from_binary::<StakerInfoResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::StakerInfo {
                staker: "addr0000".to_string(),
                staking_token: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        staker_response,
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(3666666u128),
                bond_amount: Uint128::zero(),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // query our unbonded orders, we have a total of 75, but limited to
    // 30 entries by default
    let unbonded = from_binary::<UnbondOrdersResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::UnbondOrders {
                staker: "addr0000".to_string(),
                staking_token: "lp00001".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(unbonded.orders.len(), 30);

    // claim all of our rewards
    let msg = ExecuteMsg::ClaimRewards {
        staking_token: Some("lp00001".to_string()),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(3666666u128), // 1,000,000 * 10 / (10 + 20)
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // verify pending rewards gone, bond amount is zero, withdrawable amount is zero
    let staker_response = from_binary::<StakerInfoResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::StakerInfo {
                staker: "addr0000".to_string(),
                staking_token: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        staker_response,
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::zero(),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // fast forward until everything is available to be unbonded
    env.block.time = env.block.time.plus_seconds(200000);

    // withdrawable amount only shows 5000, should be 7500, but we're limited
    // by MAX_ORDER_WITHDRAW_PER_TX
    let staker_response = from_binary::<StakerInfoResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::StakerInfo {
                staker: "addr0000".to_string(),
                staking_token: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        staker_response,
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::zero(),
                withdrawable_amount: Uint128::from(5000u128),
            }]
        }
    );

    // first claim unbonded - this will only claim 50/75 orders
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(5000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_unbonded"),
            attr("staking_token", "lp00001"),
            attr("staker", "addr0000"),
            attr("amount", "5000"),
        ]
    );

    // query staker response, withdrawable amount now shows full remaining 2500
    let staker_response = from_binary::<StakerInfoResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::StakerInfo {
                staker: "addr0000".to_string(),
                staking_token: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        staker_response,
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::zero(),
                withdrawable_amount: Uint128::from(2500u128),
            }]
        }
    );

    // query our unbonded orders, we now have 25
    let unbonded = from_binary::<UnbondOrdersResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::UnbondOrders {
                staker: "addr0000".to_string(),
                staking_token: "lp00001".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(unbonded.orders.len(), 25);

    // second claim unbonded - this will claim all 25 orders
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(2500u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_unbonded"),
            attr("staking_token", "lp00001"),
            attr("staker", "addr0000"),
            attr("amount", "2500"),
        ]
    );

    // query staker info, reward infos has been removed
    let staker_response = from_binary::<StakerInfoResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::StakerInfo {
                staker: "addr0000".to_string(),
                staking_token: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        staker_response,
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![]
        }
    );

    // query unbond orders, empty
    let unbonded = from_binary::<UnbondOrdersResponse>(
        &query(
            deps.as_ref(),
            env,
            QueryMsg::UnbondOrders {
                staker: "addr0000".to_string(),
                staking_token: "lp00001".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(unbonded.orders.len(), 0);
}

#[test]
fn test_claim_unbonded_after_claiming_rewards() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("addr0000", &[]);
    let mut env = mock_env();
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        prism_token: "prism0000".to_string(),
        distribution_schedule: default_distribution_schedule(),
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64, 200u64),
            ("lp00002".to_string(), 20u64, 200u64),
        ],
    };
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // check staker info
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::from(0u128),
                bond_amount: Uint128::from(100u128),
                withdrawable_amount: Uint128::zero(),
            }]
        }
    );

    // unbond at T=100
    let info = mock_info("addr0000", &[]);
    env.block.time = env.block.time.plus_seconds(10);
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(100u128)),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 0);

    // claim rewards at T=500, this is after the unbond period, need to
    // make sure that after we claim rewards, our subsequent ClaimUnbonded works
    let msg = ExecuteMsg::ClaimRewards {
        staking_token: None,
    };
    env.block.time = env.block.time.plus_seconds(400);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(33333u128),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    // we still have a pending withdraw, so rewards should not be cleaned yet
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                staking_token: "lp00001".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::zero(),
                withdrawable_amount: Uint128::from(100u128),
            }]
        }
    );

    // unbond orders is empty
    assert_eq!(
        from_binary::<UnbondOrdersResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::UnbondOrders {
                    staker: "addr0000".to_string(),
                    staking_token: "lp00001".to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        UnbondOrdersResponse {
            withdrawable_amount: Uint128::zero(),
            orders: vec![]
        }
    );

    // claim unbonded, we get our bonded 100 back
    let msg = ExecuteMsg::ClaimUnbonded {
        staking_token: "lp00001".to_string(),
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_unbonded"),
            attr("staking_token", "lp00001"),
            attr("staker", "addr0000"),
            attr("amount", "100"),
        ]
    );

    // rewards are now empty
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    staking_token: None,
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_infos: vec![]
        }
    );
}
