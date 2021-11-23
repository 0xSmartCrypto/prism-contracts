use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, MemoryStorage, OwnedDeps,
    StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use terra_cosmwasm::create_swap_msg;

use crate::contract::{execute, instantiate, query};
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::yasset_staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardAssetWhitelistResponse, RewardInfoResponse, StakingMode,
};

pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let msg = InstantiateMsg {
        vault: "vault0000".to_string(),
        gov: "gov0000".to_string(),
        reward_denom: "uluna".to_string(),
        collector: "collector0000".to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        cluna_token: "cluna0000".to_string(),
        yluna_token: "yluna0000".to_string(),
        pluna_token: "pluna0000".to_string(),
        prism_token: "prism0000".to_string(),
        withdraw_fee: Decimal::percent(1),
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn test_bond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond { mode: None }).unwrap(),
    });

    // wrong token
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // valid token and mode
    let info = mock_info("yluna0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", "alice0000"),
            attr("amount", "1000000"),
            attr("mode", "Default")
        ]
    );

    // try to query staker info
    let res: RewardInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::RewardInfo {
                staker_addr: "alice0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res.clone(),
        RewardInfoResponse {
            staker_addr: "alice0000".to_string(),
            staked_amount: Uint128::from(1000000u128),
            staking_mode: None,
            ..res
        }
    );
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond { mode: None }).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info.clone(), msg).unwrap();

    // unbond more then bond amount
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(1000001u128)),
    };
    let info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("can not unbond more than the bonded amount")
    );

    // unbond half
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(500001u128)),
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unbond"),
            attr("staker_addr", "alice0000".to_string()),
            attr("amount", "500001"),
            attr("withdraw_fee", "0"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: Uint128::from(500001u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // unbond remaining
    let msg = ExecuteMsg::Unbond { amount: None };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unbond"),
            attr("staker_addr", "alice0000".to_string()),
            attr("amount", "499999"),
            attr("withdraw_fee", "0"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: Uint128::from(499999u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // other user has nothing to unbond
    let info = mock_info("bob0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, StdError::generic_err("no tokens bonded"));
}

#[test]
fn test_change_bond_mode() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond { mode: None }).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info.clone(), msg).unwrap();

    // change mode

    // expect error, can only change when bond amount is zero
    let update_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(2000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            mode: Some(StakingMode::XPrism),
        })
        .unwrap(),
    });
    let err = execute(
        deps.as_mut(),
        mock_env(),
        yluna_info.clone(),
        update_msg.clone(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("mode can only be changed if nothing is bonded")
    );

    // unbond everything
    let msg = ExecuteMsg::Unbond { amount: None };
    let info = mock_info("alice0000", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // now mode can be updated
    let res = execute(deps.as_mut(), mock_env(), yluna_info, update_msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", "alice0000"),
            attr("amount", "2000000"),
            attr("mode", "XPrism")
        ]
    );

    let res: RewardInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::RewardInfo {
                staker_addr: "alice0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res.clone(),
        RewardInfoResponse {
            staker_addr: "alice0000".to_string(),
            staked_amount: Uint128::from(2000000u128),
            staking_mode: Some(StakingMode::XPrism),
            ..res
        }
    );
}

#[test]
pub fn test_process_delegator_rewards() {
    let mut deps = mock_dependencies(&[
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(1000u128),
        },
        Coin {
            denom: "ukrw".to_string(),
            amount: Uint128::new(100u128),
        },
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::new(150u128),
        },
        Coin {
            denom: "mnt".to_string(),
            amount: Uint128::new(50u128),
        },
        Coin {
            denom: "uinr".to_string(),
            amount: Uint128::new(5000u128),
        },
    ]);

    init(&mut deps);

    let info = mock_info("vault0000", &[]);
    let msg = ExecuteMsg::ProcessDelegatorRewards {};

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(create_swap_msg(
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::new(1000u128),
                },
                "uluna".to_string()
            )),
            SubMsg::new(create_swap_msg(
                Coin {
                    denom: "ukrw".to_string(),
                    amount: Uint128::new(100u128)
                },
                "uluna".to_string()
            )),
            SubMsg::new(create_swap_msg(
                Coin {
                    denom: "uinr".to_string(),
                    amount: Uint128::new(5000u128)
                },
                "uluna".to_string()
            )),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::LunaToPylunaHook {}).unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_luna_to_cluna_hook() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uluna".to_string(),
        amount: Uint128::new(150u128),
    }]);
    init(&mut deps);

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let msg = ExecuteMsg::LunaToPylunaHook {};

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "luna_to_pyluna_hook")]);
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "vault0000".to_string(),
                msg: to_binary(&VaultExecuteMsg::BondSplit { validator: None }).unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::from(150u128),
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::DepositMintedPylunaHook {}).unwrap(),
                funds: vec![],
            })),
        ]
    )
}

#[test]
fn test_deposit_minted_pyluna_hook() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_token_balances(&[
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        ),
        (
            &"pluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        ),
    ]);

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let msg = ExecuteMsg::DepositMintedPylunaHook {};

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![attr("action", "deposit_minted_pyluna_hook")]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::DepositRewards {
                assets: vec![
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked("yluna0000".to_string()),
                        },
                        amount: Uint128::from(1000000u128),
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked("pluna0000".to_string()),
                        },
                        amount: Uint128::from(1000000u128),
                    },
                ]
            })
            .unwrap(),
            funds: vec![],
        }))]
    )
}

#[test]
fn test_whitelist() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // by default yluna and pluna are whitelisted
    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("pluna0000".to_string())
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("yluna0000".to_string())
                }
            ]
        }
    );

    // whitelist one more

    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Token {
            contract_addr: Addr::unchecked("mir0000".to_string()),
        },
    };

    // unauth attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // valid attempt
    let info = mock_info("gov0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist_reward_asset"),
            attr("whitelisted_asset", "mir0000"),
        ]
    );

    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("pluna0000".to_string())
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("yluna0000".to_string())
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("mir0000".to_string())
                }
            ]
        }
    );

    // try to register native asset
    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("only token assets can be registered")
    )
}

#[test]
fn test_internal_deposit_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(2000000u128));

    // try non whitelisted asset
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(100u128),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("mir0000".to_string()),
            },
        }],
    };

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("asset mir0000 is not whitelisted")
    );

    // deposit when bond amount is zero

    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(1000u128),
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("yluna0000".to_string()),
                },
            },
            Asset {
                amount: Uint128::from(1000u128),
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("pluna0000".to_string()),
                },
            },
        ],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "deposit_rewards")]);
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "collector0000".to_string(),
                    amount: Uint128::from(1000u128), // everything sent to collector
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "pluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "collector0000".to_string(),
                    amount: Uint128::from(1000u128), // everything sent to collector
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // bond yluna
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128), // now 50% of the total yLuna is staked
        msg: to_binary(&Cw20HookMsg::Bond { mode: None }).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit reward again
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(5000u128),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("yluna0000".to_string()),
            },
        }],
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "collector0000".to_string(),
                amount: Uint128::from(2750u128), // 10% of 50% of 5k + 50% of 2500
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    let res: PoolInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                asset_token: "yluna0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PoolInfoResponse {
            asset_token: "yluna0000".to_string(),
            reward_index: Decimal::from_ratio(2250u128, 1000000u128), // ((50% of 5k) - 250) / 1000000
        }
    );
}

#[test]
fn test_external_deposit_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Token {
            contract_addr: Addr::unchecked("mir0000".to_string()),
        },
    };
    let info = mock_info("gov0000", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // the difference with internal deposit, is that tokens need to be transfered first
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(1000u128),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("mir0000".to_string()),
            },
        }],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "deposit_rewards")]);
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mir0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: Uint128::from(1000u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mir0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "collector0000".to_string(),
                    amount: Uint128::from(1000u128), // everything, because no bonded yLuna
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond { mode: None }).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit 100 reward
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(100u128),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("yluna0000".to_string()),
            },
        }],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let res: RewardInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::RewardInfo {
                staker_addr: "alice0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "alice0000".to_string(),
            staked_amount: Uint128::from(1000000u128),
            staking_mode: None,
            rewards: vec![
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("pluna0000".to_string())
                    },
                    amount: Uint128::from(0u128)
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("yluna0000".to_string())
                    },
                    amount: Uint128::from(90u128)
                }
            ]
        }
    );

    let msg = ExecuteMsg::ClaimRewards {};

    // try execute claim from address without bonded tokens
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("no tokens bonded"));

    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "90yluna0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: Uint128::from(90u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    )
}

#[test]
fn test_claim_rewards_xprism_mode() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            mode: Some(StakingMode::XPrism),
        })
        .unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit 100 reward
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(100u128),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("yluna0000".to_string()),
            },
        }],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let res: RewardInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::RewardInfo {
                staker_addr: "alice0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "alice0000".to_string(),
            staked_amount: Uint128::from(1000000u128),
            staking_mode: Some(StakingMode::XPrism),
            rewards: vec![
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("pluna0000".to_string())
                    },
                    amount: Uint128::from(0u128)
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("yluna0000".to_string())
                    },
                    amount: Uint128::from(90u128)
                }
            ]
        }
    );

    let msg = ExecuteMsg::ClaimRewards {};

    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "90yluna0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: "collector0000".to_string(),
                    amount: Uint128::from(90u128),
                    expires: None,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "collector0000".to_string(),
                msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                    assets: vec![Asset {
                        info: AssetInfo::Token {
                            contract_addr: Addr::unchecked("yluna0000")
                        },
                        amount: Uint128::from(90u128),
                    }],
                    receiver: Some("alice0000".to_string()),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    )
}
