use crate::{
    contract::{execute, instantiate, query},
    state::{DistributionStatus, RewardInfo},
};
use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::{
    from_binary,
    testing::{mock_env, mock_info},
    to_binary, Addr, CosmosMsg, Decimal, StdError, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use prism_common::testing::mock_querier::{mock_dependencies, MOCK_CONTRACT_ADDR};
use prism_protocol::launch_pool::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, VestingStatusResponse,
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
        from_binary::<DistributionStatus>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatus {
            total_distributed: Uint128::from(500000u128),
            total_bond_amount: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::from_ratio(500000u128, 100u128),
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
        from_binary::<DistributionStatus>(
            &query(deps.as_ref(), env, QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatus {
            total_distributed: Uint128::from(500000u128),
            total_bond_amount: Uint128::zero(),
            pending_reward: Uint128::from(500000u128),
            reward_index: Decimal::zero(),
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
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // correct token
    let info = mock_info("ylunatoken0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "ylunatoken0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "ylunastaking0000".to_string(),
                amount: Uint128::from(100u128),
                msg: to_binary(&StakingHookMsg::Bond { mode: None }).unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    assert_eq!(
        from_binary::<RewardInfo>(
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
        RewardInfo {
            index: Decimal::zero(),
            pending_reward: Uint128::zero(),
        }
    );

    assert_eq!(
        from_binary::<DistributionStatus>(
            &query(deps.as_ref(), mock_env(), QueryMsg::DistributionStatus {},).unwrap(),
        )
        .unwrap(),
        DistributionStatus {
            total_distributed: Uint128::zero(),
            total_bond_amount: Uint128::from(100u128),
            pending_reward: Uint128::zero(),
            reward_index: Decimal::zero(),
        }
    );
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
    assert_eq!(err, StdError::generic_err("There are no claimable rewards"));

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
    assert_eq!(err, StdError::generic_err("unauthorized"));

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
                            info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("yluna0000"),
                            },
                            amount: Uint128::from(100u128),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("pluna0000"),
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
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("yluna0000"),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("pluna0000"),
                },
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
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // correct address
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
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
    )
}
