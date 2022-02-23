use crate::{
    contract::{execute, instantiate, query},
    error::ContractError,
};

use cosmwasm_std::{
    from_binary,
    testing::{mock_env, mock_info},
    to_binary, Addr, Decimal, StdError, Timestamp, Uint128,
};

use cw20::Cw20ReceiveMsg;
use prism_common::testing::mock_querier::mock_dependencies;

use prism_protocol::xprism_boost::{
    Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, UserInfo,
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
        boost_per_hour: Decimal::from_ratio(10u128, 1u128),
        max_boost_per_xprism: Uint128::from(100u128),
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let cfg: Config = from_binary(&res).unwrap();
    assert_eq!(
        cfg,
        Config {
            owner: Addr::unchecked("owner"),
            xprism_token: Addr::unchecked("xprism"),
            boost_per_hour: Decimal::from_ratio(10u128, 1u128),
            max_boost_per_xprism: Uint128::from(100u128),
        }
    );

    // try updating config by a malicious user
    let evil = mock_info("evil", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("me".to_string()),
        boost_per_hour: Some(Decimal::from_ratio(10000u128, 1u128)),
        max_boost_per_xprism: Some(Uint128::from(100u128)),
    };

    let err = execute(deps.as_mut(), mock_env(), evil, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // try updating config with real user
    let good = mock_info("owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("new_owner".to_string()),
        boost_per_hour: Some(Decimal::from_ratio(10000u128, 1u128)),
        max_boost_per_xprism: Some(Uint128::from(101u128)),
    };

    execute(deps.as_mut(), mock_env(), good, msg).unwrap();
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let cfg: Config = from_binary(&res).unwrap();
    assert_eq!(
        cfg,
        Config {
            owner: Addr::unchecked("new_owner"),
            xprism_token: Addr::unchecked("xprism"),
            boost_per_hour: Decimal::from_ratio(10000u128, 1u128),
            max_boost_per_xprism: Uint128::from(101u128),
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
        boost_per_hour: Decimal::from_ratio(10u128, 1u128),
        max_boost_per_xprism: Uint128::from(100u128),
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // wrong sender
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "not xprism".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("addr", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // check bond works
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(100u64);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let xprism_info = mock_info("xprism", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
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
            initial_bond: 100u64,
        }
    );

    // try unbonding more than exists
    let user_info = mock_info("user", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(101u128)),
    };
    let err = execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidUnbond {});

    // unbond some, make sure its still there
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(99u128)),
    };
    execute(deps.as_mut(), env.clone(), user_info.clone(), msg).unwrap();
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap();
    let user_boost: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_boost,
        UserInfo {
            amt_bonded: Uint128::from(1u128),
            total_boost: Uint128::zero(),
            last_updated: 100u64,
            initial_bond: 100u64,
        }
    );

    // unbond full amount, make sure query fails
    let msg = ExecuteMsg::Unbond { amount: None };
    execute(deps.as_mut(), env.clone(), user_info, msg).unwrap();
    let err = query(
        deps.as_ref(),
        env,
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap_err();
    assert_eq!(
        err,
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
        owner: "owner".to_string(),
        xprism_token: "xprism".to_string(),
        boost_per_hour: Decimal::from_ratio(100u128, 1u128),
        max_boost_per_xprism: Uint128::from(100u128),
    };

    let info = mock_info("addr", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0u64);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let xprism_info = mock_info("xprism", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();

    // every 3600 seconds, we should get 100 * 100 boost
    env.block.time = Timestamp::from_seconds(3600u64);
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap();
    let user_info: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_info,
        UserInfo {
            amt_bonded: Uint128::from(100u128),
            total_boost: Uint128::from(10000u128),
            last_updated: 3600u64,
            initial_bond: 0u64,
        }
    );

    // try bonding more and make sure boost doesn't change
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: Uint128::from(1u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let xprism_info = mock_info("xprism", &[]);
    execute(deps.as_mut(), env.clone(), xprism_info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap();
    let user_info: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_info,
        UserInfo {
            amt_bonded: Uint128::from(101u128),
            total_boost: Uint128::from(10000u128),
            last_updated: 3600u64,
            initial_bond: 0u64,
        }
    );

    // try unbonding and make sure boost goes to 0
    let user = mock_info("user", &[]);
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(99u128)),
    };
    execute(deps.as_mut(), env.clone(), user.clone(), msg).unwrap();

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap();
    let user_info: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_info,
        UserInfo {
            amt_bonded: Uint128::from(2u128),
            total_boost: Uint128::from(0u128),
            last_updated: 3600u64,
            initial_bond: 3600u64,
        }
    );

    // wait a bunch of time and make sure we hit max boost
    // max is 100 * amt_bonded
    env.block.time = Timestamp::from_seconds(10000000000u64);
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap();
    let user_info: UserInfo = from_binary(&res).unwrap();
    assert_eq!(
        user_info,
        UserInfo {
            amt_bonded: Uint128::from(2u128),
            total_boost: Uint128::from(200u128),
            last_updated: 10000000000u64,
            initial_bond: 3600u64,
        }
    );

    // unbond and make sure everything is gone
    let msg = ExecuteMsg::Unbond { amount: None };
    execute(deps.as_mut(), env.clone(), user, msg).unwrap();
    let err = query(
        deps.as_ref(),
        env,
        QueryMsg::GetBoost {
            user: Addr::unchecked("user"),
        },
    )
    .unwrap_err();
    assert_eq!(
        err,
        StdError::NotFound {
            kind: "prism_protocol::xprism_boost::UserInfo".into()
        }
    );
}
