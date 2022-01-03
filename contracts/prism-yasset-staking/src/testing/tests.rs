use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, MemoryStorage, OwnedDeps,
    StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::yasset_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponse, StateResponse,
};

const OWNER: &str = "owner";
const YASSET_TOKEN: &str = "yluna0000";
const REWARD_DISTRIBUTION: &str = "reward_distribution";

pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let msg = InstantiateMsg {
        yasset_token: YASSET_TOKEN.to_string(),
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // try to query staker info
    let _res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        reward_distribution_contract: REWARD_DISTRIBUTION.to_string(),
    };
    let info = mock_info(OWNER, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
}

#[test]
pub fn test_init() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        yasset_token: YASSET_TOKEN.to_string(),
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // query config
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();

    let expected_result = ConfigResponse {
        owner: OWNER.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        reward_distribution_contract: None,
    };
    assert_eq!(res, expected_result);

    // Unauthorized - post-initialize as random user
    let msg = ExecuteMsg::PostInitialize {
        reward_distribution_contract: REWARD_DISTRIBUTION.to_string(),
    };
    let info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // successful post-initialize as owner
    let msg = ExecuteMsg::PostInitialize {
        reward_distribution_contract: REWARD_DISTRIBUTION.to_string(),
    };
    let info = mock_info(OWNER, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // query config after post-initialize
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();

    let expected_result = ConfigResponse {
        owner: OWNER.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        reward_distribution_contract: Some(REWARD_DISTRIBUTION.to_string()),
    };
    assert_eq!(res, expected_result);

    // DuplicatePostInitialize - retry post-initialize
    let msg = ExecuteMsg::PostInitialize {
        reward_distribution_contract: REWARD_DISTRIBUTION.to_string(),
    };
    let info = mock_info(OWNER, &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicatePostInitialize {});
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
    assert_eq!(err, ContractError::Unauthorized {});

    // valid token
    let info = mock_info("yluna0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
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
    execute(deps.as_mut(), mock_env(), yluna_info.clone(), msg).unwrap();

    // unbond more then bond amount
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint128::from(1000001u128)),
    };
    let info = mock_info("alice0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidUnbond {
            reason: "can not unbond more than the bonded amount".to_string(),
        }
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
    assert_eq!(
        err,
        ContractError::InvalidUnbond {
            reason: "no tokens bonded".to_string(),
        }
    );
}

#[test]
fn test_deposit_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let luna_asset = Asset {
        amount: Uint128::from(1000u128),
        info: AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
    };

    let mir_asset = Asset {
        amount: Uint128::from(1000u128),
        info: AssetInfo::Token {
            contract_addr: Addr::unchecked("mir0000"),
        },
    };

    // Unauthorized - deposit rewards must be called form reward_distribution contract
    let info = mock_info("random_addr", &[]);
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![luna_asset.clone(), mir_asset.clone()],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // ZeroBondedAmount - deposit rewards should only be called when we have
    // something bonded
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: luna_asset.info.to_string(),
            amount: luna_asset.amount,
        }],
    );
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![luna_asset.clone(), mir_asset.clone()],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::ZeroBondedAmount {});

    // bond some yasset
    let bond_amount = Uint128::from(1000000u128);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &YASSET_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
    )]);

    // InvalidNativeFunds - need to send native funds with deposit rewards msg
    let info = mock_info(REWARD_DISTRIBUTION, &[]);
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![luna_asset.clone(), mir_asset.clone()],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::InvalidNativeFunds {});

    // successful deposit
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: luna_asset.info.to_string(),
            amount: luna_asset.amount,
        }],
    );
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![luna_asset.clone(), mir_asset.clone()],
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "deposit_rewards")]);
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mir_asset.info.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: REWARD_DISTRIBUTION.to_string(),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
                amount: mir_asset.amount,
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
                asset_info: luna_asset.info.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PoolInfoResponse {
            asset_info: luna_asset.info,
            reward_index: Decimal::from_ratio(luna_asset.amount, bond_amount),
        }
    );

    let res: PoolInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                asset_info: mir_asset.info.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PoolInfoResponse {
            asset_info: mir_asset.info,
            reward_index: Decimal::from_ratio(mir_asset.amount, bond_amount),
        }
    );
}

#[test]
fn test_deposit_rewards_multi_user() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let luna_asset = Asset {
        amount: Uint128::from(1000u128),
        info: AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
    };

    let mir_asset = Asset {
        amount: Uint128::from(1000u128),
        info: AssetInfo::Token {
            contract_addr: Addr::unchecked("mir0000"),
        },
    };

    // alice bonds 2500
    let bond_amount = Uint128::from(2500u128);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // bob bonds 2500
    let bond_amount = Uint128::from(2500u128);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "bob0000".to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // update yasset_staking balances to reflect newly bonded yassets
    deps.querier.with_token_balances(&[(
        &YASSET_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(5000u128))],
    )]);

    // deposit reward of 1000 luna and 1000 mir
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: luna_asset.info.to_string(),
            amount: luna_asset.amount,
        }],
    );
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![luna_asset.clone(), mir_asset.clone()],
    };
    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    let state: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(state.total_bond_amount, Uint128::from(5000u128));

    // query pool info for luna and mir
    let res: PoolInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                asset_info: luna_asset.info.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PoolInfoResponse {
            asset_info: luna_asset.info.clone(),
            reward_index: Decimal::from_ratio(luna_asset.amount, state.total_bond_amount),
        }
    );

    let res: PoolInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                asset_info: mir_asset.info.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PoolInfoResponse {
            asset_info: mir_asset.info,
            reward_index: Decimal::from_ratio(mir_asset.amount, state.total_bond_amount),
        }
    );

    // query pool info for anc (no rewards), should we return empy result
    // instead of erroring?
    let err = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_info: AssetInfo::Token {
                contract_addr: Addr::unchecked("anc0000"),
            },
        },
    )
    .unwrap_err();
    assert!(matches!(err, StdError::NotFound { .. }));

    // alice bonds 1000 more
    let bond_amount = Uint128::from(1000u128);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // update yasset_staking balances to reflect newly bonded yassets
    deps.querier.with_token_balances(&[(
        &YASSET_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(6000u128))],
    )]);

    let state: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(state.total_bond_amount, Uint128::from(6000u128));

    // deposit reward of 1000 luna
    let info = mock_info(
        REWARD_DISTRIBUTION,
        &[Coin {
            denom: luna_asset.info.to_string(),
            amount: luna_asset.amount,
        }],
    );
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![luna_asset.clone()],
    };
    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    // current rewards state
    // alice:   luna:   (1000 * 2500 bonded / 5000 total bonded) +
    //                  (1000 * 3500 bonded / 6000 total bonded) = 1083 uluna
    //          mir:    (1000 * 2500 bonded / 5000 total bonded) = 500 mir
    // bob:     luna:   (1000 * 2500 bonded / 5000 total bonded) +
    //                  (1000 * 2500 bonded / 6000 total bonded) = 916 uluna
    //          mir:    (1000 * 2500 bonded / 5000 total bonded) = 500 mir

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
            staked_amount: Uint128::from(3500u128),
            rewards: vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string()
                    },
                    amount: Uint128::from(1083u128)
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("mir0000".to_string())
                    },
                    amount: Uint128::from(500u128)
                }
            ]
        }
    );

    let res: RewardInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::RewardInfo {
                staker_addr: "bob0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "bob0000".to_string(),
            staked_amount: Uint128::from(2500u128),
            rewards: vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string()
                    },
                    amount: Uint128::from(916u128)
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("mir0000".to_string())
                    },
                    amount: Uint128::from(500u128)
                }
            ]
        }
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_vault_state(&Uint128::from(1000000u128));

    // bond 1e6
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "alice0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let yluna_info = mock_info("yluna0000", &[]);
    execute(deps.as_mut(), mock_env(), yluna_info, msg).unwrap();
    deps.querier.with_token_balances(&[(
        &YASSET_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
    )]);

    // deposit 100 reward
    let msg = ExecuteMsg::DepositRewards {
        assets: vec![Asset {
            amount: Uint128::from(100u128),
            info: AssetInfo::Token {
                contract_addr: Addr::unchecked("mir0000".to_string()),
            },
        }],
    };

    let info = mock_info(REWARD_DISTRIBUTION, &[]);
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
            rewards: vec![
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uluna".to_string()
                    },
                    amount: Uint128::from(0u128)
                },
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("mir0000".to_string())
                    },
                    amount: Uint128::from(100u128)
                }
            ]
        }
    );

    let msg = ExecuteMsg::ClaimRewards {};

    // try execute claim from address without bonded tokens
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidUnbond {
            reason: "no tokens bonded".to_string(),
        }
    );

    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_rewards"),
            attr("claimed_asset", "100mir0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mir0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "alice0000".to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // claim again, nothing returned
    let info = mock_info("alice0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "claim_rewards"),]);
    assert_eq!(res.messages, vec![],);
}
