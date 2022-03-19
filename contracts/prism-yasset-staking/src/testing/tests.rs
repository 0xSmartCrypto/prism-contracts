use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, MemoryStorage, OwnedDeps,
    StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::{Asset, AssetInfo};
use prismswap::asset::PrismSwapAssetInfo;
use std::str::FromStr;
use terra_cosmwasm::create_swap_msg;

use crate::contract::{execute, instantiate, query};
use crate::state::CONFIG;
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier, VAULT};
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::gov::Cw20HookMsg as GovCw20HookMsg;
use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardAssetWhitelistResponse, RewardInfoResponse,
};


pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        vault: VAULT.to_string(),
        gov: "gov0000".to_string(),
        collector: "collector0000".to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        cluna_token: "cluna0000".to_string(),
        yluna_token: "yluna0000".to_string(),
        pluna_token: "pluna0000".to_string(),
        prism_token: "prism0000".to_string(),
        xprism_token: "xprism0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn test_init() {
    let mut deps = mock_dependencies(&[]);

    // successful init
    init(&mut deps);

    // query config
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        res,
        ConfigResponse {
            owner: "owner0000".to_string(),
            vault: VAULT.to_string(),
            gov: "gov0000".to_string(),
            collector: "collector0000".to_string(),
            protocol_fee: Decimal::from_ratio(1u128, 10u128),
            cluna_token: "cluna0000".to_string(),
            yluna_token: "yluna0000".to_string(),
            pluna_token: "pluna0000".to_string(),
            prism_token: "prism0000".to_string(),
            xprism_token: "xprism0000".to_string(),
        }
    );
}

#[test]
fn test_bond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // wrong token
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // valid token
    let info = mock_info("yluna0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", "alice0000"),
            attr("amount", "1000000"),
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
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

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

    let info = mock_info(VAULT, &[]);
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
                contract_addr: VAULT.to_string(),
                msg: to_binary(&VaultExecuteMsg::BondSplit { validator: None }).unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::from(150u128),
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::DepositMintedPylunaHook {
                    prev_pluna_balance: Uint128::zero(),
                    prev_yluna_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    )
}

#[test]
fn test_deposit_minted_pyluna_hook() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let msg = ExecuteMsg::DepositMintedPylunaHook {
        prev_pluna_balance: Uint128::from(600000u128),
        prev_yluna_balance: Uint128::from(700000u128),
    };

    deps.querier.with_token_balances(&[
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        ),
        (
            &"pluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(2000000u128))],
        ),
    ]);

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
                        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                        amount: Uint128::from(300000u128), // 1000000 - 700000
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                        amount: Uint128::from(1400000u128), // 2000000 - 600000
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
                AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            ]
        }
    );

    // whitelist one more

    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("mir0000")),
    };

    // unauth attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // valid attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist_reward_asset"),
            attr("whitelisted_asset", "cw20:mir0000"),
        ]
    );

    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                AssetInfo::Cw20(Addr::unchecked("mir0000")),
            ]
        }
    );

    // try to register native asset
    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Native("uusd".to_string()),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("only token assets can be whitelisted")
    );

    // remove whiteslited asset
    let msg = ExecuteMsg::RemoveRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
    };

    // unauth attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // valid attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "remove_whitelisted_reward_asset"),
            attr("removed_asset", "cw20:yluna0000"),
        ]
    );

    // verify whitelist is modified
    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                AssetInfo::Cw20(Addr::unchecked("mir0000")),
            ]
        }
    );

    // try to remove non whitelisted asset
    let msg = ExecuteMsg::RemoveRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("random0000")),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, StdError::generic_err("this asset is not whitelisted"));
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
            info: AssetInfo::Cw20(Addr::unchecked("mir0000")),
        }],
    };

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("asset cw20:mir0000 is not whitelisted")
    );

    // deposit when bond amount is zero

    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(1000u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
            Asset {
                amount: Uint128::from(1000u128),
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            },
        ],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
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
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit reward again
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(5000u128),
            info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
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
        asset: AssetInfo::Cw20(Addr::unchecked("mir0000")),
    };
    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // the difference with internal deposit, is that tokens need to be transfered first
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(1000u128),
            info: AssetInfo::Cw20(Addr::unchecked("mir0000")),
        }],
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
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
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit 100 reward
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(100u128),
            info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        }],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            rewards: vec![
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                    amount: Uint128::from(0u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(90u128)
                }
            ]
        }
    );

    // claim rewards as yluna
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
            attr("claimed_asset", "cw20:yluna0000:90"),
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
fn test_convert_and_claim_rewards_prism() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit rewards - 100 yluna, 500 pLuna
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            },
            Asset {
                amount: Uint128::from(100u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
        ],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            rewards: vec![
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                    amount: Uint128::from(450u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(90u128)
                }
            ]
        }
    );

    // claim rewards as prism, should result in the following:
    // 1 - increase allowance of 90 yluna0000 for collector
    // 1 - increase allowance of 450 pluna0000 for collector
    // 3 - call ConvertAndSend on collector with 90 yluna and 450 pluna
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };
    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "cw20:pluna0000:450"),
            attr("claimed_asset", "cw20:yluna0000:90"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(450u128),
                expires: None,
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
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
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "collector0000".to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: vec![
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                        amount: Uint128::from(450u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                        amount: Uint128::from(90u128),
                    },
                ],
                receiver: Some(info.sender.to_string()),
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
}

#[test]
fn test_convert_and_claim_rewards_yluna() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    // whitelist anchor as a reward asset
    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("anc0000")),
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist_reward_asset"),
            attr("whitelisted_asset", "cw20:anc0000"),
        ]
    );

    // bond some yluna
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit rewards - 100 yluna, 500 pLuna, 750 anc
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            },
            Asset {
                amount: Uint128::from(100u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
            Asset {
                amount: Uint128::from(750u128),
                info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
            },
        ],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            rewards: vec![
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                    amount: Uint128::from(450u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(90u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
                    amount: Uint128::from(675u128)
                }
            ]
        }
    );

    // claim rewards as yluna, should result in the following:
    // 1 - increase allowance of 450 pluna for collector
    // 2 - transfer yluna directly to collector
    // 3 - increase allowance of 675 anc for collector
    // 4 - call ConvertAndSend on collector with 90 yluna, 450 pluna, 675 anc
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
    };
    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "cw20:pluna0000:450"),
            attr("claimed_asset", "cw20:yluna0000:90"),
            attr("claimed_asset", "cw20:anc0000:675"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(450u128),
                expires: None,
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(90u128),
                recipient: info.sender.to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(675u128),
                expires: None,
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "collector0000".to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: vec![
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                        amount: Uint128::from(450u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
                        amount: Uint128::from(675u128),
                    },
                ],
                receiver: Some(info.sender.to_string()),
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
}

#[test]
fn test_convert_and_claim_rewards_xprism() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit rewards - 100 yluna, 500 pLuna
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            },
            Asset {
                amount: Uint128::from(100u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
        ],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            rewards: vec![
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                    amount: Uint128::from(450u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(90u128)
                }
            ]
        }
    );

    // claim rewards as xprism, should result in the following:
    // 1 - increase allowance of 90 yluna0000 for collector
    // 1 - increase allowance of 450 pluna0000 for collector
    // 3 - call ConvertAndSend on collector with 90 yluna and 450 pluna with
    //       recipient set to MOCK_CONTRACT_ADDR
    // 4 - call MintXPrismHook on yasset-staking with recipient set to sender
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Cw20(Addr::unchecked("xprism0000")),
    };
    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 4);
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "cw20:pluna0000:450"),
            attr("claimed_asset", "cw20:yluna0000:90"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(450u128),
                expires: None,
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
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
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "collector0000".to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: vec![
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                        amount: Uint128::from(450u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                        amount: Uint128::from(90u128),
                    },
                ],
                receiver: Some(MOCK_CONTRACT_ADDR.to_string()),
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::MintXprismClaimHook {
                receiver: info.sender,
                prev_balance: Uint128::zero(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    // same as above but with some current prism balance
    // given the current code, our contract should not have any prism balance,
    // but we're using that logic just in case or for any future refactors
    deps.querier.with_token_balances(&[(
        &"prism0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000u128))],
    )]);

    // deposit rewards - 100 yluna, 500 pLuna
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(100u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            },
        ],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Cw20(Addr::unchecked("xprism0000")),
    };
    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 4);

    // note that I'm only checking the last message here to verify prev balance
    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::MintXprismClaimHook {
                receiver: info.sender,
                prev_balance: Uint128::from(1000u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_mint_xprism_claim_hook() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // create hook message with prev balance = 0
    let msg = ExecuteMsg::MintXprismClaimHook {
        receiver: Addr::unchecked("addr0000"),
        prev_balance: Uint128::zero(),
    };

    // set the balance to 1000 to simulate the rewards conversion
    deps.querier.with_token_balances(&[(
        &"prism0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000u128))],
    )]);

    // unauthorized
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // success, we mint xprism with the full 1000 prism balance
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "gov0000".to_string(),
                amount: Uint128::from(1000u128),
                msg: to_binary(&GovCw20HookMsg::MintXprism {
                    receiver: Some("addr0000".to_string()),
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
            attr("action", "mint_xprism_claim_hook"),
            attr("prism_amount_to_mint_xprism", "1000"),
        ]
    );

    // one more time with prev balance = 250, should only mint with 750 prism here
    let msg = ExecuteMsg::MintXprismClaimHook {
        receiver: Addr::unchecked("addr0000"),
        prev_balance: Uint128::from(250u128),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "gov0000".to_string(),
                amount: Uint128::from(750u128),
                msg: to_binary(&GovCw20HookMsg::MintXprism {
                    receiver: Some("addr0000".to_string()),
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
            attr("action", "mint_xprism_claim_hook"),
            attr("prism_amount_to_mint_xprism", "750"),
        ]
    );
}

#[test]
fn test_convert_and_claim_rewards_invalid_claim() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit rewards - 100 yluna, 500 pLuna
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(100u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            },
        ],
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            rewards: vec![
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
                    amount: Uint128::from(450u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(90u128)
                }
            ]
        }
    );

    // claim rewards as uusd, invalid - only prism, xprism, cluna, pluna, yluna supported
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Native("uusd".to_string()),
    };
    let info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Native claim assets not supported")
    );

    // claim rewards as anc, invalid - only prism, xprism, cluna, pluna, yluna supported
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Cw20(Addr::unchecked("anc0000")),
    };
    let info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, StdError::generic_err("Claim asset not supported"));
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // Unauthorized user is unable to update anything.
    {
        let msg = ExecuteMsg::UpdateConfig {
            owner: Some(String::from("mallory666")),
            collector: Some(String::from("mallory666")),
            protocol_fee: Some(Decimal::from_ratio(1u128, 2u128)),
        };
        let info = mock_info("mallory666", &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, StdError::generic_err("unauthorized"));
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, "owner0000");
        assert_eq!(config.collector, "collector0000");
        assert_eq!(config.protocol_fee, Decimal::from_ratio(1u128, 10u128));
    }

    // Authorized user with blank input updates nothing.
    {
        let blank_msg = ExecuteMsg::UpdateConfig {
            owner: None,
            collector: None,
            protocol_fee: None,
        };
        let info = mock_info("owner0000", &[]);
        execute(deps.as_mut(), mock_env(), info, blank_msg).unwrap();
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, "owner0000");
        assert_eq!(config.collector, "collector0000");
        assert_eq!(config.protocol_fee, Decimal::from_ratio(1u128, 10u128));
    }

    // Setting protocol fee too high causes error (even when valid owner).
    {
        let msg = ExecuteMsg::UpdateConfig {
            owner: None,
            collector: None,
            protocol_fee: Some(Decimal::from_str("0.500000000000000001").unwrap()),
        };
        let info = mock_info("owner0000", &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            err,
            StdError::generic_err("fee can not be greater than 0.5")
        );
    }

    // Authorized user is able to update everything.
    {
        let full_msg = ExecuteMsg::UpdateConfig {
            owner: Some(String::from("new-owner")),
            collector: Some(String::from("new-collector")),
            protocol_fee: Some(Decimal::from_ratio(1u128, 2u128)),
        };
        let info = mock_info("owner0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, full_msg).unwrap();
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, "new-owner");
        assert_eq!(config.collector, "new-collector");
        assert_eq!(config.protocol_fee, Decimal::from_ratio(1u128, 2u128));
        assert_eq!(res.attributes, vec![attr("action", "update_config")]);
    }
}

#[test]
fn test_asset_serialization() {
    // verify that a cw20 asset info is serialized as the address of the token
    let asset_info = AssetInfo::Cw20(Addr::unchecked("addr0000"));
    let asset_token_addr = Addr::unchecked("addr0000");
    let asset_token_string = "addr0000".to_string();

    assert_eq!(asset_info.as_bytes(), asset_token_addr.as_bytes());
    assert_eq!(asset_info.as_bytes(), asset_token_string.as_bytes());
}
