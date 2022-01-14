use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Api, BankMsg, Coin, CosmosMsg, Decimal, OwnedDeps, Querier,
    Storage, SubMsg, Uint128, WasmMsg,
};

use astroport::asset::{Asset, AssetInfo};
use cosmwasm_std::testing::{mock_env, mock_info};

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use prism_protocol::reward_distribution::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, RewardAssetWhitelistResponse,
};

use cw20::Cw20ExecuteMsg;

use prism_common::testing::mock_querier::{
    mock_dependencies, MOCK_CONTRACT_ADDR, VAULT, YASSET_STAKING, YASSET_STAKING_X,
};
use prism_protocol::yasset_staking::ExecuteMsg as StakingExecuteMsg;
use terra_cosmwasm::create_swap_msg;

const OWNER: &str = "owner";
const GOV: &str = "gov";
const YASSET_TOKEN: &str = "ybeth";
const COLLECTOR: &str = "collector";
const DELEGATOR_REWARD_DENOM: &str = "uluna";

pub fn init<S: Storage, A: Api, Q: Querier>(deps: &mut OwnedDeps<S, A, Q>) {
    let msg = InstantiateMsg {
        vault: VAULT.to_string(),
        gov: GOV.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        yasset_staking: YASSET_STAKING.to_string(),
        yasset_staking_x: YASSET_STAKING_X.to_string(),
        collector: COLLECTOR.to_string(),
        delegator_reward_denom: DELEGATOR_REWARD_DENOM.to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        whitelisted_assets: vec![AssetInfo::NativeToken {
            denom: DELEGATOR_REWARD_DENOM.to_string(),
        }],
    };

    let owner_info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();
}

#[test]
fn test_initialization() {
    let mut deps = mock_dependencies(&[]);

    // invalid protocol fee
    let msg = InstantiateMsg {
        vault: VAULT.to_string(),
        gov: GOV.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        yasset_staking: YASSET_STAKING.to_string(),
        yasset_staking_x: YASSET_STAKING_X.to_string(),
        collector: COLLECTOR.to_string(),
        delegator_reward_denom: DELEGATOR_REWARD_DENOM.to_string(),
        protocol_fee: Decimal::from_ratio(11u128, 10u128),
        whitelisted_assets: vec![AssetInfo::NativeToken {
            denom: DELEGATOR_REWARD_DENOM.to_string(),
        }],
    };

    let owner_info = mock_info(OWNER, &[]);
    let res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap_err();
    assert_eq!(
        res,
        ContractError::InvalidConfig {
            reason: "invalid protocol fee".to_string()
        }
    );

    // invalid whitelist assets
    let msg = InstantiateMsg {
        vault: VAULT.to_string(),
        gov: GOV.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        yasset_staking: YASSET_STAKING.to_string(),
        yasset_staking_x: YASSET_STAKING_X.to_string(),
        collector: COLLECTOR.to_string(),
        delegator_reward_denom: DELEGATOR_REWARD_DENOM.to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        whitelisted_assets: vec![],
    };

    let owner_info = mock_info(OWNER, &[]);
    let res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap_err();
    assert_eq!(
        res,
        ContractError::InvalidConfig {
            reason: "whitelisted assets cannot be empty".to_string()
        }
    );

    // valid init
    init(&mut deps);

    // verify config storage
    let state = QueryMsg::Config {};
    let config_response: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = ConfigResponse {
        vault: VAULT.to_string(),
        gov: GOV.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        yasset_staking: YASSET_STAKING.to_string(),
        yasset_staking_x: YASSET_STAKING_X.to_string(),
        collector: COLLECTOR.to_string(),
        delegator_reward_denom: DELEGATOR_REWARD_DENOM.to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
    };
    assert_eq!(config_response, expected_result);
}

#[test]
fn test_process_delegator_rewards() {
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

    let info = mock_info("random_user", &[]);
    let msg = ExecuteMsg::ProcessDelegatorRewards {};

    // unauthorized error - only vault can call process delegator rewards
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // success
    let info = mock_info(VAULT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(create_swap_msg(
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::new(1000u128),
                },
                DELEGATOR_REWARD_DENOM.to_string()
            )),
            SubMsg::new(create_swap_msg(
                Coin {
                    denom: "ukrw".to_string(),
                    amount: Uint128::new(100u128)
                },
                DELEGATOR_REWARD_DENOM.to_string()
            )),
            SubMsg::new(create_swap_msg(
                Coin {
                    denom: "uinr".to_string(),
                    amount: Uint128::new(5000u128)
                },
                DELEGATOR_REWARD_DENOM.to_string()
            )),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::DistributeRewards {
                    asset_infos: vec![AssetInfo::NativeToken {
                        denom: DELEGATOR_REWARD_DENOM.to_string()
                    }]
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_distribute_rewards_native() {
    let mut deps = mock_dependencies(&[]);

    init(&mut deps);
    let info = mock_info("random_user", &[]);

    let reward_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        amount: Uint128::from(100u128),
    };

    // set contract balance=100 - this is the reward that will be distributed
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::new(100u128),
        },
    )]);

    let msg = ExecuteMsg::DistributeRewards {
        asset_infos: vec![reward_asset.info.clone()],
    };

    // unauthorized error - only vault can distribute rewards
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // empty vault err
    let info = mock_info(VAULT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::EmptyVault {});

    // vault = 1000
    // yasset_bonded = 0
    // yasset_x_bonded = 0
    // reward = 100
    // collector gets 100
    // yasset_staking gets 0
    // yasset_staking_x gets 0
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: COLLECTOR.to_string(),
            amount: vec![Coin {
                denom: reward_asset.info.to_string(),
                amount: Uint128::new(100u128),
            },]
        }))]
    );

    // vault = 1000
    // yasset_staking = 300
    // yasset_staking_x = 0
    // reward = 100
    // collector gets 73
    // yasset_staking gets 27
    // yasset_staking_x gets 0
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    deps.querier
        .with_yasset_staking_state(&Uint128::from(300u128));
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: COLLECTOR.to_string(),
                amount: vec![Coin {
                    denom: reward_asset.info.to_string(),
                    amount: Uint128::from(73u128),
                },]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_STAKING.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![Asset {
                        amount: Uint128::from(27u128),
                        ..reward_asset.clone()
                    }],
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: reward_asset.info.to_string(),
                    amount: Uint128::from(27u128)
                }],
            }))
        ]
    );

    // vault = 1000
    // yasset_staking = 300
    // yasset_staking_x = 500
    // reward = 100
    // collector gets 28
    // yasset_staking gets 27
    // yasset_staking_x gets 45
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    deps.querier
        .with_yasset_staking_state(&Uint128::from(300u128));
    deps.querier
        .with_yasset_staking_x_state(&Uint128::from(500u128));
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: COLLECTOR.to_string(),
                amount: vec![Coin {
                    denom: reward_asset.info.to_string(),
                    amount: Uint128::from(28u128),
                },]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_STAKING.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![Asset {
                        amount: Uint128::from(27u128),
                        ..reward_asset.clone()
                    }],
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: reward_asset.info.to_string(),
                    amount: Uint128::from(27u128)
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_STAKING_X.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![Asset {
                        amount: Uint128::from(45u128),
                        ..reward_asset.clone()
                    }],
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: reward_asset.info.to_string(),
                    amount: Uint128::from(45u128)
                }],
            })),
        ]
    );
}

#[test]
fn test_distribute_rewards_token() {
    let mut deps = mock_dependencies(&[]);

    init(&mut deps);
    let info = mock_info(VAULT, &[]);

    let reward_asset = Asset {
        info: AssetInfo::Token {
            contract_addr: Addr::unchecked("ANC"),
        },
        amount: Uint128::from(100u128),
    };

    // set contract balance=100 anc tokens - this is the reward that will be distributed
    deps.querier.with_token_balances(&[(
        &reward_asset.info.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &reward_asset.amount)],
    )]);

    let msg = ExecuteMsg::DistributeRewards {
        asset_infos: vec![reward_asset.info.clone()],
    };

    // failure - ANC not whitelisted
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::RewardAssetNotWhitelisted {
            asset: "ANC".to_string()
        }
    );

    // whitelist ANC
    let whitelist_msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Token {
            contract_addr: Addr::unchecked("ANC".to_string()),
        },
    };
    let info = mock_info(GOV, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), whitelist_msg).unwrap();

    // vault = 1000
    // yasset_bonded = 0
    // yasset_x_bonded = 0
    // reward = 100
    // collector gets 100
    // yasset_staking gets 0
    // yasset_staking_x gets 0
    let info = mock_info(VAULT, &[]);
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: reward_asset.info.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: COLLECTOR.to_string(),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // vault = 1000
    // yasset_staking = 300
    // yasset_staking_x = 0
    // reward = 100
    // collector gets 73
    // yasset_staking gets 27
    // yasset_staking_x gets 0
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    deps.querier
        .with_yasset_staking_state(&Uint128::from(300u128));
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_asset.info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: COLLECTOR.to_string(),
                    amount: Uint128::from(73u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_asset.info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: YASSET_STAKING.to_string(),
                    amount: Uint128::from(27u128),
                    expires: None,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_STAKING.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![Asset {
                        amount: Uint128::from(27u128),
                        ..reward_asset.clone()
                    }],
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    // vault = 1000
    // yasset_staking = 300
    // yasset_staking_x = 500
    // reward = 100
    // collector gets 28
    // yasset_staking gets 27
    // yasset_staking_x gets 45
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    deps.querier
        .with_yasset_staking_state(&Uint128::from(300u128));
    deps.querier
        .with_yasset_staking_x_state(&Uint128::from(500u128));
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_asset.info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: COLLECTOR.to_string(),
                    amount: Uint128::from(28u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_asset.info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: YASSET_STAKING.to_string(),
                    amount: Uint128::from(27u128),
                    expires: None,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_STAKING.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![Asset {
                        amount: Uint128::from(27u128),
                        ..reward_asset.clone()
                    }],
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_asset.info.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: YASSET_STAKING_X.to_string(),
                    amount: Uint128::from(45u128),
                    expires: None,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_STAKING_X.to_string(),
                msg: to_binary(&StakingExecuteMsg::DepositRewards {
                    assets: vec![Asset {
                        amount: Uint128::from(45u128),
                        ..reward_asset
                    }],
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_whitelist() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // uluna is whitelisted inside the init method
    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![AssetInfo::NativeToken {
                denom: "uluna".to_string()
            },]
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
    assert_eq!(err, ContractError::Unauthorized {});

    // valid attempt
    let info = mock_info(GOV, &[]);
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
                AssetInfo::NativeToken {
                    denom: "uluna".to_string()
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("mir0000".to_string())
                }
            ]
        }
    );

    // whitelist native asset
    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist_reward_asset"),
            attr("whitelisted_asset", "uusd"),
        ]
    );

    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::NativeToken {
                    denom: "uluna".to_string()
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("mir0000".to_string())
                },
                AssetInfo::NativeToken {
                    denom: "uusd".to_string()
                },
            ]
        }
    );

    // distribute rewards for non-whitelisted asset
    let reward_asset = Asset {
        info: AssetInfo::Token {
            contract_addr: Addr::unchecked("ANC"),
        },
        amount: Uint128::from(100u128),
    };

    // set contract balance=100 anc tokens - this is the reward that will be distributed
    deps.querier.with_token_balances(&[(
        &reward_asset.info.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &reward_asset.amount)],
    )]);

    let msg = ExecuteMsg::DistributeRewards {
        asset_infos: vec![reward_asset.info.clone()],
    };
    let info = mock_info(VAULT, &[]);

    // failure - ANC not whitelisted
    deps.querier.with_vault_state(&Uint128::from(1000u128));
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::RewardAssetNotWhitelisted {
            asset: "ANC".to_string()
        }
    );

    // whitelist ANC
    let whitelist_msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Token {
            contract_addr: Addr::unchecked("ANC".to_string()),
        },
    };
    let info = mock_info(GOV, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), whitelist_msg).unwrap();

    // successful distribute rewards
    let info = mock_info(VAULT, &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
}
