use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Coin, ContractResult, CosmosMsg, Decimal, MemoryStorage,
    OwnedDeps, Reply, ReplyOn, SubMsg, SubMsgExecutionResponse, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use cw_asset::{Asset, AssetInfo};

use crate::contract::{execute, instantiate, query, reply};
use cw20_base::msg::InstantiateMsg as TokenInstantiateMsg;
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::collector::ExecuteMsg as CollectorExecuteMsg;
use prism_protocol::yasset_staking_x::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
};

use crate::error::ContractError;

const OWNER: &str = "owner";
const BOB: &str = "bob";
const YASSET_TOKEN: &str = "yasset_token";
const XYASSET_TOKEN: &str = "xyasset_token";
const PRISM_TOKEN: &str = "prism_token";
const PRISM_YASSET_PAIR: &str = "prism_yasset_pair";
const COLLECTOR: &str = "collector";
const REWARD_DISTRIBUTION: &str = "reward_distribution";

pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let msg = InstantiateMsg {
        yasset_token: YASSET_TOKEN.to_string(),
        prism_token: PRISM_TOKEN.to_string(),
        prism_yasset_pair: PRISM_YASSET_PAIR.to_string(),
        collector: COLLECTOR.to_string(),
        reward_distribution: REWARD_DISTRIBUTION.to_string(),
        token_code_id: 3,
    };

    let owner_info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    let reply_msg = get_token_instantiate_reply_msg();
    reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
}

fn get_token_instantiate_reply_msg() -> Reply {
    Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(
                // https://developers.google.com/protocol-buffers/docs/encoding
                // byte notes:
                // 10 = 1010
                //      field number 1 from response proto file (contract_address)
                //      wire type 010 => 2 => length-delim
                // 13 = length of remaining characters
                // 120, 121, ... 110 = "xyasset_token" in ascii
                vec![
                    10, 13, 120, 121, 97, 115, 115, 101, 116, 95, 116, 111, 107, 101, 110,
                ]
                .into(),
            ),
        }),
    }
}

#[test]
fn test_init() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        yasset_token: YASSET_TOKEN.to_string(),
        prism_token: PRISM_TOKEN.to_string(),
        prism_yasset_pair: PRISM_YASSET_PAIR.to_string(),
        collector: COLLECTOR.to_string(),
        reward_distribution: REWARD_DISTRIBUTION.to_string(),
        token_code_id: 3,
    };

    let owner_info = mock_info(OWNER, &[]);
    let res = instantiate(deps.as_mut(), mock_env(), owner_info, msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: msg.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: "xpLuna".to_string(),
                    symbol: "xpLUNA".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: MOCK_CONTRACT_ADDR.to_string(),
                        cap: None,
                    }),
                    marketing: None,
                })
                .unwrap(),
                funds: vec![],
                label: "".to_string(),
                admin: None,
            }
            .into(),
            gas_limit: None,
            id: 1,
            reply_on: ReplyOn::Success,
        }]
    );

    let reply_msg = get_token_instantiate_reply_msg();
    reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    // verify config storage
    let state = QueryMsg::Config {};
    let config_response: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = ConfigResponse {
        owner: OWNER.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        xyasset_token: XYASSET_TOKEN.to_string(),
        prism_token: PRISM_TOKEN.to_string(),
        collector: COLLECTOR.to_string(),
        reward_distribution: REWARD_DISTRIBUTION.to_string(),
    };
    assert_eq!(config_response, expected_result);

    // verify state storage
    let state = QueryMsg::State {};
    let state_response: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = StateResponse {
        total_bond_amount: Uint128::zero(),
        yasset_balance: Uint128::zero(),
        exchange_rate: Decimal::one(),
    };
    assert_eq!(state_response, expected_result);
}

// bond once for 1000
#[test]
fn test_bond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // query state, nothing bonded yet
    let res: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(
        res,
        StateResponse {
            total_bond_amount: Uint128::zero(),
            yasset_balance: Uint128::zero(),
            exchange_rate: Decimal::one(),
        }
    );

    let bond_amount = Uint128::from(1000u128);
    let staker = BOB;

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // unauthorized
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // successful bond, need to set balance on contract to emulate the CW20 send
    deps.querier.with_token_balances(&[(
        &YASSET_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &bond_amount)],
    )]);
    let info = mock_info(YASSET_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let expected_mint_amount = bond_amount;
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: XYASSET_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: staker.to_string(),
                amount: expected_mint_amount,
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", staker.to_string()),
            attr("amount", bond_amount),
            attr("mint_amount", expected_mint_amount)
        ]
    );

    // set balances
    deps.querier.with_token_balances(&[
        (
            &YASSET_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &bond_amount)],
        ),
        (
            &XYASSET_TOKEN.to_string(),
            &[(&staker.to_string(), &expected_mint_amount)],
        ),
    ]);

    // query state
    let res: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(
        res,
        StateResponse {
            total_bond_amount: bond_amount,
            yasset_balance: expected_mint_amount,
            exchange_rate: Decimal::one(),
        }
    );
}

// bond once for 1000, add reward of 500, bond again for another 1000,
// should receive 666 minted xyasset on second bond.
#[test]
fn test_bond_exchange_rate() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let bond_amount = Uint128::from(1000u128);
    let staker = BOB;

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // successful bond, need to set balance on contract to emulate the CW20 send
    deps.querier.with_token_balances(&[(
        &YASSET_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &bond_amount)],
    )]);
    let info = mock_info(YASSET_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    let expected_mint_amount = bond_amount;
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: XYASSET_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: staker.to_string(),
                amount: expected_mint_amount,
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", staker.to_string()),
            attr("amount", bond_amount),
            attr("mint_amount", expected_mint_amount)
        ]
    );

    // set balances, also add 500 to yasset as a reward, exchange rate will
    // now be 1000/1500 = .667
    let reward = Uint128::from(500u128);
    let yasset_balance_with_reward = bond_amount + reward;
    deps.querier.with_token_balances(&[
        (
            &YASSET_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &yasset_balance_with_reward)],
        ),
        (
            &XYASSET_TOKEN.to_string(),
            &[(&staker.to_string(), &expected_mint_amount)],
        ),
    ]);

    // query state
    let res: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(
        res,
        StateResponse {
            total_bond_amount: bond_amount,
            yasset_balance: yasset_balance_with_reward,
            exchange_rate: Decimal::from_ratio(yasset_balance_with_reward, bond_amount),
        }
    );

    // bond another 1000, this with with 2/3 exchange rate, so we'll get back 666
    let yasset_balance_with_new_bond = yasset_balance_with_reward + bond_amount;
    deps.querier.with_token_balances(&[
        (
            &YASSET_TOKEN.to_string(),
            &[(
                &MOCK_CONTRACT_ADDR.to_string(),
                &yasset_balance_with_new_bond,
            )],
        ),
        (
            &XYASSET_TOKEN.to_string(),
            &[(&staker.to_string(), &expected_mint_amount)],
        ),
    ]);

    let info = mock_info(YASSET_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let expected_mint_amount = Uint128::from(666u128); // ensure we round down
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: XYASSET_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: staker.to_string(),
                amount: expected_mint_amount,
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", staker.to_string()),
            attr("amount", bond_amount),
            attr("mint_amount", expected_mint_amount)
        ]
    );
}

// unbond 175 at 2/3 exchange rate, get 262 yasset back
#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let staker = BOB;
    let unbond_amount = Uint128::from(175u128);

    // configure balances for exchange_rate = 2/3
    let yasset_balance = Uint128::from(1500u128);
    let xyasset_supply = Uint128::from(1000u128);
    deps.querier.with_token_balances(&[
        (
            &YASSET_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &yasset_balance)],
        ),
        (
            &XYASSET_TOKEN.to_string(),
            &[(&staker.to_string(), &xyasset_supply)],
        ),
    ]);

    // unbond 300 at exchange rate = 2/3, we should get back 450 xyassets
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: unbond_amount,
        msg: to_binary(&Cw20HookMsg::Unbond {}).unwrap(),
    });

    // unauthorized
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    let info = mock_info(XYASSET_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let expected_redeem_amount = Uint128::from(262u128); // ensure we round down
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: XYASSET_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: unbond_amount,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_TOKEN.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: staker.to_string(),
                    amount: expected_redeem_amount,
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unbond"),
            attr("staker_addr", staker.to_string()),
            attr("amount", unbond_amount),
            attr("redeem_amount", expected_redeem_amount)
        ]
    );
}

#[test]
fn test_deposit_rewards_native() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let reward_assets = vec![Asset {
        info: AssetInfo::Native("uusd".to_string()),
        amount: Uint128::from(1000u128),
    }];

    // Unauthorized - deposit rewards must be called form reward_distribution contract
    let sent_coin = Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(1001u128),
    };
    let info = mock_info("random_addr", &[sent_coin]);
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // Invalid native funds - nothing sent
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::InvalidNativeFunds {});

    // Invalid native funds - denom mismatch
    let sent_coin = Coin {
        denom: "ukrw".to_string(),
        amount: Uint128::from(1000u128),
    };
    let info = mock_info(REWARD_DISTRIBUTION, &[sent_coin]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::InvalidNativeFunds {});

    // Invalid native funds - amount mismatch
    let sent_coin = Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(1001u128),
    };
    let info = mock_info(REWARD_DISTRIBUTION, &[sent_coin]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::InvalidNativeFunds {});

    // successful deposit
    let sent_coin = Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(1000u128),
    };
    let info = mock_info(REWARD_DISTRIBUTION, &[sent_coin]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: COLLECTOR.to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: reward_assets,
                receiver: None,
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked(YASSET_TOKEN)),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    // two native tokens - Invalid native funds
    let reward_assets = vec![
        Asset {
            info: AssetInfo::Native("uusd".to_string()),
            amount: Uint128::from(1000u128),
        },
        Asset {
            info: AssetInfo::Native("uluna".to_string()),
            amount: Uint128::from(500u128),
        },
    ];
    let sent_coins = vec![
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000u128),
        },
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(1000u128),
        },
    ];
    let info = mock_info(REWARD_DISTRIBUTION, &sent_coins);
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::InvalidNativeFunds {});

    // two native tokens - success
    let sent_coins = vec![
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000u128),
        },
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(500u128),
        },
    ];
    let info = mock_info(REWARD_DISTRIBUTION, &sent_coins);
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: COLLECTOR.to_string(),
            msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                assets: reward_assets,
                receiver: None,
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked(YASSET_TOKEN)),
            })
            .unwrap(),
            funds: vec![],
        })),]
    )
}

#[test]
fn test_deposit_rewards_token() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let reward_assets = vec![Asset {
        info: AssetInfo::Cw20(Addr::unchecked("ANC")),
        amount: Uint128::from(1000u128),
    }];

    // successful deposit
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ANC".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: mock_env().contract.address.to_string(),
                    amount: reward_assets[0].amount,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: COLLECTOR.to_string(),
                msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                    assets: reward_assets,
                    receiver: None,
                    dest_asset_info: AssetInfo::Cw20(Addr::unchecked(YASSET_TOKEN)),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    // two tokens - success
    let reward_assets = vec![
        Asset {
            info: AssetInfo::Cw20(Addr::unchecked("ANC")),
            amount: Uint128::from(1000u128),
        },
        Asset {
            info: AssetInfo::Cw20(Addr::unchecked("MIR")),
            amount: Uint128::from(500u128),
        },
    ];
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ANC".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: mock_env().contract.address.to_string(),
                    amount: reward_assets[0].amount,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "MIR".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: mock_env().contract.address.to_string(),
                    amount: reward_assets[1].amount,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: COLLECTOR.to_string(),
                msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                    assets: reward_assets,
                    receiver: None,
                    dest_asset_info: AssetInfo::Cw20(Addr::unchecked(YASSET_TOKEN)),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    )
}

#[test]
fn test_deposit_rewards_native_and_token() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let reward_assets = vec![
        Asset {
            info: AssetInfo::Native("uusd".to_string()),
            amount: Uint128::from(2000u128),
        },
        Asset {
            info: AssetInfo::Cw20(Addr::unchecked("ANC")),
            amount: Uint128::from(1000u128),
        },
    ];

    // Invalid native funds - nothing sent
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::InvalidNativeFunds {});

    // success
    let sent_coin = Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(2000u128),
    };
    let info = mock_info(REWARD_DISTRIBUTION, &[sent_coin]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ANC".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: mock_env().contract.address.to_string(),
                    amount: reward_assets[1].amount,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: COLLECTOR.to_string(),
                msg: to_binary(&CollectorExecuteMsg::ConvertAndSend {
                    assets: reward_assets,
                    receiver: None,
                    dest_asset_info: AssetInfo::Cw20(Addr::unchecked(YASSET_TOKEN)),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_deposit_rewards_no_convert() {
    // testing depositing yasset_token, no need for ConvertAndSend message
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // test depositing yasset_token
    let reward_assets = vec![Asset {
        info: AssetInfo::Cw20(Addr::unchecked(YASSET_TOKEN)),
        amount: Uint128::from(1000u128),
    }];

    let info = mock_info(REWARD_DISTRIBUTION, &[]);
    let msg = ExecuteMsg::DepositRewards {
        assets: reward_assets.clone(),
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: YASSET_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: mock_env().contract.address.to_string(),
                amount: reward_assets[0].amount,
            })
            .unwrap(),
            funds: vec![],
        })),]
    );
}
