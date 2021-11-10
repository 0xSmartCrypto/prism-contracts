use crate::contract::{execute, instantiate, query};
use crate::ContractError;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, to_binary, CosmosMsg, Decimal, SubMsg, Uint128, WasmMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use prism_protocol::lp_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponseItem, StakerInfoResponse, StakersInfoResponse,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64),
            ("lp00002".to_string(), 20u64),
        ],
    };

    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            prism_token: "prism0000".to_string(),
            distribution_schedule: vec![(100, 200, Uint128::from(1000000u128))],
            staking_tokens: vec![
                ("lp00001".to_string(), 10u64),
                ("lp00002".to_string(), 20u64)
            ],
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
            reward_index: Decimal::zero(),
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
            reward_index: Decimal::zero(),
        }
    );
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![
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
        ],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64),
            ("lp00002".to_string(), 20u64),
        ],
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            reward_index: Decimal::zero(),
            last_distributed: default_genesis_seconds,
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
            reward_index: Decimal::from_ratio(33333u128, 100u128),
            last_distributed: default_genesis_seconds + 10,
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
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![
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
        ],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64),
            ("lp00002".to_string(), 20u64),
        ],
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("lp00001", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(150u128)),
    };

    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidUnbondAmount {});

    // normal unbond
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: Some(Uint128::from(60u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(60u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // unbond remaining
    let msg = ExecuteMsg::Unbond {
        staking_token: "lp00001".to_string(),
        amount: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "lp00001".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(40u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_compute_reward() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![
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
        ],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64),
            ("lp00002".to_string(), 20u64),
        ],
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            }]
        }
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![
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
        ],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64),
            ("lp00002".to_string(), 20u64),
        ],
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

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
fn test_query_stakers() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        prism_token: "prism0000".to_string(),
        distribution_schedule: vec![
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
        ],
        staking_tokens: vec![
            ("lp00001".to_string(), 10u64),
            ("lp00002".to_string(), 20u64),
        ],
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

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
                    }]
                },
                StakerInfoResponse {
                    staker: "addr0001".to_string(),
                    reward_infos: vec![RewardInfoResponseItem {
                        staking_token: "lp00001".to_string(),
                        pending_reward: Uint128::zero(),
                        bond_amount: Uint128::from(200u128),
                    }]
                },
                StakerInfoResponse {
                    staker: "addr0002".to_string(),
                    reward_infos: vec![RewardInfoResponseItem {
                        staking_token: "lp00001".to_string(),
                        pending_reward: Uint128::zero(),
                        bond_amount: Uint128::from(300u128),
                    }]
                }
            ]
        }
    );
    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
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
                }]
            },]
        }
    );
}
