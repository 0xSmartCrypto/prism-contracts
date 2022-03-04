use crate::{
    contract::{execute, instantiate, migrate, query},
    error::ContractError,
};
use cosmwasm_std::CosmosMsg;
use cosmwasm_std::{
    from_binary,
    testing::{mock_env, mock_info},
    to_binary, Addr, Decimal, Env, Response, StdError, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use prism_common::testing::mock_querier::mock_dependencies;
use prism_protocol::launch_pool::ExecuteMsg as LaunchPoolExecuteMsg;
use prism_protocol::xprism_boost::{
    Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UserInfo,
};

// 1. Test instantiate and update config from bad person
// 2. Test instantiate and update config from
#[test]
fn test_config_updates() {
    let mut deps = mock_dependencies(&[]);
    // 10 boost per hour
    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        xprism_token: "xprism".to_string(),
        boost_per_hour: Decimal::one(),
        max_boost_per_xprism: Uint128::from(100u128),
        launch_pool_contract: Some("launch-pool0000".to_string()),
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_binary::<Config>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: Addr::unchecked("owner"),
            xprism_token: Addr::unchecked("xprism"),
            boost_per_hour: Decimal::one(),
            max_boost_per_xprism: Uint128::from(100u128),
            launch_pool_contract: Some(Addr::unchecked("launch-pool0000")),
        }
    );

    // try updating config with invalid boost_per_hour
    let owner = mock_info("owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        boost_per_hour: Some(Decimal::from_ratio(11u128, 10u128)),
        max_boost_per_xprism: Some(Uint128::from(100u128)),
        launch_pool_contract: None,
    };

    let err = execute(deps.as_mut(), mock_env(), owner, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidBoostInterval {});

    // try updating config by a malicious user
    let evil = mock_info("evil", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("me".to_string()),
        boost_per_hour: Some(Decimal::from_ratio(1u128, 2u128)),
        max_boost_per_xprism: Some(Uint128::from(100u128)),
        launch_pool_contract: None,
    };

    let err = execute(deps.as_mut(), mock_env(), evil, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // try updating config with real user
    let good = mock_info("owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("new_owner".to_string()),
        boost_per_hour: Some(Decimal::from_ratio(1u128, 2u128)),
        max_boost_per_xprism: Some(Uint128::from(101u128)),
        launch_pool_contract: None,
    };

    execute(deps.as_mut(), mock_env(), good, msg).unwrap();
    assert_eq!(
        from_binary::<Config>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: Addr::unchecked("new_owner"),
            xprism_token: Addr::unchecked("xprism"),
            boost_per_hour: Decimal::from_ratio(1u128, 2u128),
            max_boost_per_xprism: Uint128::from(101u128),
            launch_pool_contract: Some(Addr::unchecked("launch-pool0000")),
        }
    );

    // Change launch_pool address to something else.
    let good = mock_info("new_owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        boost_per_hour: None,
        max_boost_per_xprism: None,
        launch_pool_contract: Some("something-else0000".to_string()),
    };

    execute(deps.as_mut(), mock_env(), good, msg).unwrap();
    assert_eq!(
        from_binary::<Config>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: Addr::unchecked("new_owner"),
            xprism_token: Addr::unchecked("xprism"),
            boost_per_hour: Decimal::from_ratio(1u128, 2u128),
            max_boost_per_xprism: Uint128::from(101u128),
            launch_pool_contract: Some(Addr::unchecked("something-else0000")),
        }
    );

    // Clear launch_pool address.
    let good = mock_info("new_owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        boost_per_hour: None,
        max_boost_per_xprism: None,
        launch_pool_contract: Some("".to_string()),
    };

    execute(deps.as_mut(), mock_env(), good, msg).unwrap();
    assert_eq!(
        from_binary::<Config>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: Addr::unchecked("new_owner"),
            xprism_token: Addr::unchecked("xprism"),
            boost_per_hour: Decimal::from_ratio(1u128, 2u128),
            max_boost_per_xprism: Uint128::from(101u128),
            launch_pool_contract: None,
        }
    );
}

// 3. Bond from non xprism token, fail
// 4. Bond with xprism token, make sure query returns right info
// 6. Bond, unbond more to force fail
// 7. Bond, unbond some, check query is still there
// 5. Bond, unbond same amount, make sure query fails
#[test]
fn test_basic_bonding() {
    let mut deps = mock_dependencies(&[]);
    // 10 boost per hour
    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        xprism_token: "xprism".to_string(),
        boost_per_hour: Decimal::one(),
        max_boost_per_xprism: Uint128::from(100u128),
        launch_pool_contract: None,
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Sad path: wrong sender (only xPRISM contract can call this).
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "not xprism".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond { user: None }).unwrap(),
    });

    let info = mock_info("addr", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Happy path: bond and check query shows that amount as bonded.
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(100u64);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond { user: None }).unwrap(),
    });

    let xprism_info = mock_info("xprism", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();
    assert_eq!(
        from_binary::<UserInfo>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::GetBoost {
                    user: Addr::unchecked("user"),
                },
            )
            .unwrap()
        )
        .unwrap(),
        UserInfo {
            amt_bonded: Uint128::from(100u128),
            total_boost: Uint128::zero(),
            last_updated: 100u64,
            boost_accrual_start_time: 100u64,
        }
    );

    // Sad path: Try unbonding more than exists.
    let user_info = mock_info("user", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(101u128)),
    };
    let err = execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidUnbond {});

    // Happy path: unbond some.
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(99u128)),
    };
    execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    // make sure query still shows amt_bonded is still > 0.
    assert_eq!(
        from_binary::<UserInfo>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::GetBoost {
                    user: Addr::unchecked("user"),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        UserInfo {
            amt_bonded: Uint128::from(1u128),
            total_boost: Uint128::zero(),
            last_updated: 100u64,
            boost_accrual_start_time: 100u64,
        }
    );

    // Happy path: unbond full amount. Unbound call should work, but GetBoost
    // should now start returning 404s.
    let msg = ExecuteMsg::Unbond { amount: None };
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    assert_eq!(
        query(
            deps.as_ref(),
            env,
            QueryMsg::GetBoost {
                user: Addr::unchecked("user"),
            },
        )
        .unwrap_err(),
        StdError::NotFound {
            kind: "prism_protocol::xprism_boost::UserInfo".into()
        }
    );
}

// 8. Bond then wait 1hr and check boost
// 9. try bonding more and make sure boost doesn't change
// 10. try unbonding and make sure boost goes to 0
// 11. bond some, wait a billion years and make sure we clamp boost
#[test]
fn test_boost_updates() {
    let mut deps = mock_dependencies(&[]);
    // 100 boost per hour
    let msg = InstantiateMsg {
        owner: "owner000".to_string(),
        xprism_token: "xprism000".to_string(),
        boost_per_hour: Decimal::from_ratio(1u128, 2u128),
        max_boost_per_xprism: Uint128::from(100u128),
        launch_pool_contract: Some("launch-pool000".to_string()),
    };

    let info = mock_info("admin000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0u64);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond { user: None }).unwrap(),
    });

    let xprism_info = mock_info("xprism000", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();

    // every 3600 seconds, we should get 100 * 0.5 boost
    env.block.time = Timestamp::from_seconds(3600u64);
    assert_eq!(
        from_binary::<UserInfo>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::GetBoost {
                    user: Addr::unchecked("user000"),
                },
            )
            .unwrap()
        )
        .unwrap(),
        UserInfo {
            amt_bonded: Uint128::from(100u128),
            total_boost: Uint128::from(50u128),
            last_updated: 3600u64,
            boost_accrual_start_time: 0u64,
        }
    );

    // try bonding more and make sure boost doesn't change
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user000".to_string(),
        amount: Uint128::from(1u128),
        msg: to_binary(&Cw20HookMsg::Bond { user: None }).unwrap(),
    });

    let xprism_info = mock_info("xprism000", &[]);
    assert_eq!(
        execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap(),
        Response::new()
            .add_attribute("user", "user000")
            .add_attribute("bond", "1"),
    );

    assert_eq!(
        from_binary::<UserInfo>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::GetBoost {
                    user: Addr::unchecked("user000"),
                },
            )
            .unwrap()
        )
        .unwrap(),
        UserInfo {
            amt_bonded: Uint128::from(101u128),
            total_boost: Uint128::from(50u128),
            last_updated: 3600u64,
            boost_accrual_start_time: 0u64,
        }
    );

    // try unbonding and make sure boost goes to 0
    let user = mock_info("user000", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(99u128)),
    };
    execute(deps.as_mut(), env.clone(), user.clone(), msg).unwrap();

    assert_eq!(
        from_binary::<UserInfo>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::GetBoost {
                    user: Addr::unchecked("user000"),
                },
            )
            .unwrap()
        )
        .unwrap(),
        UserInfo {
            amt_bonded: Uint128::from(2u128),
            total_boost: Uint128::from(0u128),
            last_updated: 3600u64,
            boost_accrual_start_time: 3600u64,
        }
    );

    // wait a bunch of time and make sure we hit max boost
    // max is 100 * amt_bonded
    env.block.time = Timestamp::from_seconds(10000000000u64);
    assert_eq!(
        from_binary::<UserInfo>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::GetBoost {
                    user: Addr::unchecked("user000"),
                },
            )
            .unwrap()
        )
        .unwrap(),
        UserInfo {
            amt_bonded: Uint128::from(2u128),
            total_boost: Uint128::from(200u128),
            last_updated: 10000000000u64,
            boost_accrual_start_time: 3600u64,
        }
    );

    // unbond and make sure everything is gone
    let msg = ExecuteMsg::Unbond { amount: None };
    execute(deps.as_mut(), env.clone(), user, msg).unwrap();
    assert_eq!(
        query(
            deps.as_ref(),
            env,
            QueryMsg::GetBoost {
                user: Addr::unchecked("user000"),
            },
        )
        .unwrap_err(),
        StdError::NotFound {
            kind: "prism_protocol::xprism_boost::UserInfo".into()
        }
    );
}

#[test]
fn test_calls_launch_pool_contract_after_unbond() {
    // Test summary:
    // - Time 0: Init contract with launch_pool_address set;
    // - Time 1h: Alice bonds some xPRISM;
    // - Time 2h: Alice unbonds a portion of her xPRISM; launch_pool_address
    //   should be called.
    // - Time 3h: Contract is updated to have launch_pool_address == None.
    // - Time 4h:  Alice unbonds another portion of her xPRISM;
    //   launch_pool_address should be NOT called.
    let mut deps = mock_dependencies(&[]);

    fn mock_env_at_time(hours: u32) -> Env {
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(hours as u64 * 3600u64);
        env
    }

    // Time 0: Init contract with launch_pool_address set;
    let msg = InstantiateMsg {
        owner: "owner000".to_string(),
        xprism_token: "xprism000".to_string(),
        boost_per_hour: Decimal::from_ratio(1u128, 1u128), // 1 per hour.
        max_boost_per_xprism: Uint128::from(3600u128),
        launch_pool_contract: Some("launch-pool000".to_string()),
    };
    let info = mock_info("admin000", &[]);
    instantiate(deps.as_mut(), mock_env_at_time(0), info, msg).unwrap();
    assert_eq!(
        from_binary::<Config>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap()
            .launch_pool_contract,
        Some(Addr::unchecked("launch-pool000")),
    );

    // Time 1h: Alice bonds some xPRISM;
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond { user: None }).unwrap(),
    });
    let xprism_info = mock_info("xprism000", &[]);
    assert_eq!(
        execute(deps.as_mut(), mock_env_at_time(1), xprism_info, msg).unwrap(),
        Response::new()
            .add_attribute("user", "alice000")
            .add_attribute("bond", "100"),
    );

    // Time 2h: Alice unbonds a portion of her xPRISM; launch_pool_address
    // should be called.
    let user = mock_info("alice000", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(20u128)),
    };
    assert_eq!(
        execute(deps.as_mut(), mock_env_at_time(2), user, msg).unwrap(),
        Response::new()
            .add_attribute("user", "alice000")
            .add_attribute("unbond", "20")
            .add_messages(vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "xprism000".to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "alice000".to_string(),
                        amount: Uint128::from(20u128),
                    })
                    .unwrap(),
                    funds: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "launch-pool000".to_string(),
                    msg: to_binary(&LaunchPoolExecuteMsg::PrivilegedRefreshBoost {
                        account: "alice000".to_string(),
                    })
                    .unwrap(),
                    funds: vec![],
                }),
            ]),
    );

    // Time 3h: Contract is updated to have launch_pool_address == None.
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        boost_per_hour: None,
        max_boost_per_xprism: None,
        launch_pool_contract: Some("".to_string()),
    };
    execute(
        deps.as_mut(),
        mock_env_at_time(3),
        mock_info("owner000", &[]),
        msg,
    )
    .unwrap();
    assert_eq!(
        from_binary::<Config>(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap())
            .unwrap()
            .launch_pool_contract,
        None,
    );

    // Time 4h:  Alice unbonds another portion of her xPRISM;
    // launch_pool_address should be NOT called.
    let user = mock_info("alice000", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(40u128)),
    };
    assert_eq!(
        execute(deps.as_mut(), mock_env_at_time(40), user, msg).unwrap(),
        Response::new()
            .add_attribute("user", "alice000")
            .add_attribute("unbond", "40")
            .add_messages(vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "xprism000".to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "alice000".to_string(),
                        amount: Uint128::from(40u128),
                    })
                    .unwrap(),
                    funds: vec![],
                }),
                // LaunchPoolExecuteMsg::PrivilegedRefreshBoost intentionally missing here.
            ]),
    );
}

#[test]
fn test_bonding_from_different_user() {
    let mut deps = mock_dependencies(&[]);
    // 10 boost per hour
    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        xprism_token: "xprism".to_string(),
        boost_per_hour: Decimal::one(),
        max_boost_per_xprism: Uint128::from(100u128),
        launch_pool_contract: Some("launch-pool-contract".to_string()),
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // user00001 bond 200 at T=100
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(100u64);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user0001".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond { user: None }).unwrap(),
    });

    let xprism_info = mock_info("xprism", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        env,
        QueryMsg::GetBoost {
            user: Addr::unchecked("user0001"),
        },
    )
    .unwrap();
    let user_info: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_info,
        UserInfo {
            amt_bonded: Uint128::from(100u128),
            total_boost: Uint128::zero(),
            last_updated: 100u64,
            boost_accrual_start_time: 100u64,
        }
    );

    // user00002 bonds 100 at T=200 on behalf of user0001
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(200u64);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user0002".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            user: Some("user0001".to_string()),
        })
        .unwrap(),
    });

    let xprism_info = mock_info("xprism", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();

    // query user0001, verify 200 bonded
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user0001"),
        },
    )
    .unwrap();
    let user_info: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_info,
        UserInfo {
            amt_bonded: Uint128::from(200u128),
            total_boost: Uint128::from(2u128),
            last_updated: 200u64,
            boost_accrual_start_time: 100u64,
        }
    );

    // query user0002, verify nothing bonded
    let err = query(
        deps.as_ref(),
        env,
        QueryMsg::GetBoost {
            user: Addr::unchecked("user0002"),
        },
    )
    .unwrap_err();
    assert!(matches!(err, StdError::NotFound { .. }));
}

#[test]
fn test_migrate() {
    let mut deps = mock_dependencies(&[]);
    // 10 boost per hour
    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        xprism_token: "xprism".to_string(),
        boost_per_hour: Decimal::one(),
        max_boost_per_xprism: Uint128::from(100u128),
        launch_pool_contract: Some("launch-pool-contract".to_string()),
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
}
