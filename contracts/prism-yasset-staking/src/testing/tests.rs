use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, MemoryStorage,
    OwnedDeps, StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::{Asset, AssetInfo};
use prismswap::asset::PrismSwapAssetInfo;

use crate::contract::{execute, instantiate, query};
use crate::state::CONFIG;
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::gov::Cw20HookMsg as GovCw20HookMsg;
use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponse, StateResponse,
};

const OWNER: &str = "owner0000";
const GOV: &str = "gov0000";
const COLLECTOR: &str = "collector0000";
const YLUNA_TOKEN: &str = "yluna0000";
const PLUNA_TOKEN: &str = "pluna0000";
const CLUNA_TOKEN: &str = "cluna0000";
const PRISM_TOKEN: &str = "prism0000";
const XPRISM_TOKEN: &str = "xprism0000";
const REWARD_DISTRIBUTION: &str = "reward_distribution0000";

pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let msg = InstantiateMsg {
        owner: OWNER.to_string(),
        gov: GOV.to_string(),
        collector: COLLECTOR.to_string(),
        yasset_token: YLUNA_TOKEN.to_string(),
        prism_token: PRISM_TOKEN.to_string(),
        xprism_token: XPRISM_TOKEN.to_string(),
        reward_distribution: REWARD_DISTRIBUTION.to_string(),
        claim_assets: vec![
            AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
            AssetInfo::Cw20(Addr::unchecked(CLUNA_TOKEN)),
            AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
            AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
            AssetInfo::Cw20(Addr::unchecked(XPRISM_TOKEN)),
        ],
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
            owner: OWNER.to_string(),
            gov: GOV.to_string(),
            collector: COLLECTOR.to_string(),
            yasset_token: YLUNA_TOKEN.to_string(),
            prism_token: PRISM_TOKEN.to_string(),
            xprism_token: XPRISM_TOKEN.to_string(),
            reward_distribution: REWARD_DISTRIBUTION.to_string(),
            claim_assets: vec![
                AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(CLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(XPRISM_TOKEN)),
            ],
        }
    );
}

#[test]
fn test_bond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1_000_000u128),
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
            staked_amount: Uint128::from(1_000_000u128),
            ..res
        }
    );

    // query bond amount
    let bond_amount: Uint128 =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::BondAmount {}).unwrap()).unwrap();
    assert_eq!(bond_amount, Uint128::from(1_000_000u128));

    // query state
    let state: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(
        state,
        StateResponse {
            total_bond_amount: Uint128::from(1_000_000u128)
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
fn test_deposit_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(1000u128),
                info: AssetInfo::Cw20(Addr::unchecked("mir0000")),
            },
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Native("uusd".to_string()),
            },
        ],
    };

    // unauthorized
    let info = mock_info("not_reward_distribution0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    let info = mock_info(REWARD_DISTRIBUTION, &[]);

    // bonded amount is zero
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, StdError::generic_err("zero bonded amount"));

    // bond yluna
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit reward again
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(5000u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Native("uusd".to_string()),
            },
        ],
    };
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(500u128),
        }],
    );

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
                amount: Uint128::from(5000u128),
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
            reward_index: Decimal::from_ratio(5000u128, 1000000u128),
        }
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));
    deps.querier.with_reward_assets(vec![
        AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
        AssetInfo::Native("uusd".to_string()),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit 100 reward of yluna and 500 of uusd
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![
            Asset {
                amount: Uint128::from(100u128),
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            },
            Asset {
                amount: Uint128::from(500u128),
                info: AssetInfo::Native("uusd".to_string()),
            },
        ],
    };
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(500u128),
        }],
    );
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
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(100u128)
                },
                Asset {
                    info: AssetInfo::Native("uusd".to_string()),
                    amount: Uint128::from(500u128)
                },
            ],
        }
    );

    // claim rewards
    let msg = ExecuteMsg::ClaimRewards {};

    // try execute claim from address without bonded tokens
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("no tokens bonded"));

    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "cw20:yluna0000:100"),
            attr("claimed_asset", "native:uusd:500"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "alice0000".to_string(),
                    amount: Uint128::from(100u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(500u128),
                }],
            })),
        ]
    )
}

#[test]
fn test_convert_and_claim_rewards_prism() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));
    deps.querier.with_reward_assets(vec![
        AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
        AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
    ]);

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
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
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
                    amount: Uint128::from(500u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(100u128)
                },
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
            attr("claimed_asset", "cw20:pluna0000:500"),
            attr("claimed_asset", "cw20:yluna0000:100"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(500u128),
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
                amount: Uint128::from(100u128),
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
                        amount: Uint128::from(500u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                        amount: Uint128::from(100u128),
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
    deps.querier.with_reward_assets(vec![
        AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
        AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
        AssetInfo::Cw20(Addr::unchecked("anc0000")),
        AssetInfo::Native("uusd".to_string()),
    ]);

    // bond some yluna
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit rewards - 100 yluna, 500 pLuna, 750 anc, 2000 uusd
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
            Asset {
                amount: Uint128::from(2000u128),
                info: AssetInfo::Native("uusd".to_string()),
            },
        ],
    };
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(2000u128),
        }],
    );
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
                    amount: Uint128::from(500u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(100u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
                    amount: Uint128::from(750u128)
                },
                Asset {
                    info: AssetInfo::Native("uusd".to_string()),
                    amount: Uint128::from(2000u128)
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
            attr("claimed_asset", "cw20:pluna0000:500"),
            attr("claimed_asset", "cw20:yluna0000:100"),
            attr("claimed_asset", "cw20:anc0000:750"),
            attr("claimed_asset", "native:uusd:2000"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(500u128),
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
                amount: Uint128::new(100u128),
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
                amount: Uint128::from(750u128),
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
                        amount: Uint128::from(500u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
                        amount: Uint128::from(750u128),
                    },
                    Asset {
                        info: AssetInfo::Native("uusd".to_string()),
                        amount: Uint128::from(2000u128),
                    },
                ],
                receiver: Some(info.sender.to_string()),
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(2000u128)
            }],
        })),
    );
}

#[test]
fn test_convert_and_claim_rewards_xprism() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));
    deps.querier.with_reward_assets(vec![
        AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
        AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
    ]);

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
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
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
                    amount: Uint128::from(500u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(100u128)
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
            attr("claimed_asset", "cw20:pluna0000:500"),
            attr("claimed_asset", "cw20:yluna0000:100"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(500u128),
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
                amount: Uint128::from(100u128),
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
                        amount: Uint128::from(500u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                        amount: Uint128::from(100u128),
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
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
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
    deps.querier.with_reward_assets(vec![
        AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
        AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
    ]);

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
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
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
                    amount: Uint128::from(500u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(100u128)
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
        StdError::generic_err("claim asset not supported: native:uusd")
    );

    // claim rewards as anc, invalid - only prism, xprism, cluna, pluna, yluna supported
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Cw20(Addr::unchecked("anc0000")),
    };
    let info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("claim asset not supported: cw20:anc0000")
    );
}

#[test]
fn test_convert_and_claim_rewards_uusd() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));
    deps.querier.with_reward_assets(vec![
        AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
        AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
        AssetInfo::Native("uusd".to_string()),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();

    // deposit rewards - 100 yluna, 500 pLuna, 1000 uusd
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
                amount: Uint128::from(1000u128),
                info: AssetInfo::Native("uusd".to_string()),
            },
        ],
    };
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000u128),
        }],
    );
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
                    amount: Uint128::from(500u128)
                },
                Asset {
                    info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                    amount: Uint128::from(100u128)
                },
                Asset {
                    info: AssetInfo::Native("uusd".to_string()),
                    amount: Uint128::from(1000u128)
                },
            ]
        }
    );

    // add uusd as a claim asset
    let msg = ExecuteMsg::AddClaimAsset {
        asset: AssetInfo::Native("uusd".to_string()),
    };
    let info = mock_info(OWNER, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 0);
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "add_claim_asset"),
            attr("claim_asset", "native:uusd"),
        ]
    );

    // claim rewards as uusd, should result in the following:
    // 1 - transfer 900 uusd to sender
    // 1 - increase allowance of 90 yluna0000 for collector
    // 1 - increase allowance of 450 pluna0000 for collector
    // 3 - call ConvertAndSend on collector with 90 yluna and 450 pluna
    let msg = ExecuteMsg::ConvertAndClaimRewards {
        claim_asset: AssetInfo::Native("uusd".to_string()),
    };
    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "cw20:pluna0000:500"),
            attr("claimed_asset", "cw20:yluna0000:100"),
            attr("claimed_asset", "native:uusd:1000"),
        ]
    );
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                spender: "collector0000".to_string(),
                amount: Uint128::from(500u128),
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
                amount: Uint128::from(100u128),
                expires: None,
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1000u128),
            }],
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
                        amount: Uint128::from(500u128),
                    },
                    Asset {
                        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                        amount: Uint128::from(100u128),
                    },
                ],
                receiver: Some(info.sender.to_string()),
                dest_asset_info: AssetInfo::Native("uusd".to_string()),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // Unauthorized user is unable to update anything.
    {
        let msg = ExecuteMsg::UpdateConfig {
            owner: Some(String::from("mallory666")),
        };
        let info = mock_info("mallory666", &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, StdError::generic_err("unauthorized"));
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, "owner0000");
    }

    // Authorized user with blank input updates nothing.
    {
        let blank_msg = ExecuteMsg::UpdateConfig { owner: None };
        let info = mock_info("owner0000", &[]);
        execute(deps.as_mut(), mock_env(), info, blank_msg).unwrap();
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, "owner0000");
    }

    // Authorized user is able to update everything.
    {
        let full_msg = ExecuteMsg::UpdateConfig {
            owner: Some(String::from("new-owner")),
        };
        let info = mock_info("owner0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, full_msg).unwrap();
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, "new-owner");
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

#[test]
fn test_add_remove_claim_asset() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // query starting config
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        res,
        ConfigResponse {
            owner: OWNER.to_string(),
            gov: GOV.to_string(),
            collector: COLLECTOR.to_string(),
            yasset_token: YLUNA_TOKEN.to_string(),
            prism_token: PRISM_TOKEN.to_string(),
            xprism_token: XPRISM_TOKEN.to_string(),
            reward_distribution: REWARD_DISTRIBUTION.to_string(),
            claim_assets: vec![
                AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(CLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(XPRISM_TOKEN)),
            ],
        }
    );

    // error - unauthorized add claim asset
    {
        let msg = ExecuteMsg::AddClaimAsset {
            asset: AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
        };
        let info = mock_info("not_the_owner0000", &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, StdError::generic_err("unauthorized"));
    }

    // error - duplicate claim asset
    {
        let msg = ExecuteMsg::AddClaimAsset {
            asset: AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
        };
        let info = mock_info(OWNER, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, StdError::generic_err("duplicate claim asset"));
    }

    // successful add claim asset
    {
        let msg = ExecuteMsg::AddClaimAsset {
            asset: AssetInfo::Native("uusd".to_string()),
        };
        let info = mock_info(OWNER, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "add_claim_asset"),
                attr("claim_asset", "native:uusd"),
            ]
        );
    }

    // error - unauthorized remove claim asset
    {
        let msg = ExecuteMsg::RemoveClaimAsset {
            asset: AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
        };
        let info = mock_info("not_the_owner0000", &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, StdError::generic_err("unauthorized"));
    }

    // error - claim asset doesn't exist
    {
        let msg = ExecuteMsg::RemoveClaimAsset {
            asset: AssetInfo::Cw20(Addr::unchecked("anc0000")),
        };
        let info = mock_info(OWNER, &[]);
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, StdError::generic_err("claim asset doesn't exist"));
    }

    // successful remove claim asset
    {
        let msg = ExecuteMsg::RemoveClaimAsset {
            asset: AssetInfo::Cw20(Addr::unchecked(CLUNA_TOKEN)),
        };
        let info = mock_info(OWNER, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "remove_claim_asset"),
                attr("claim_asset", "cw20:cluna0000"),
            ]
        );
    }

    // query final config - added uusd, removed cluna0000
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        res,
        ConfigResponse {
            owner: OWNER.to_string(),
            gov: GOV.to_string(),
            collector: COLLECTOR.to_string(),
            yasset_token: YLUNA_TOKEN.to_string(),
            prism_token: PRISM_TOKEN.to_string(),
            xprism_token: XPRISM_TOKEN.to_string(),
            reward_distribution: REWARD_DISTRIBUTION.to_string(),
            claim_assets: vec![
                AssetInfo::Cw20(Addr::unchecked(YLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(PLUNA_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(PRISM_TOKEN)),
                AssetInfo::Cw20(Addr::unchecked(XPRISM_TOKEN)),
                AssetInfo::Native("uusd".to_string()),
            ],
        }
    );
}
