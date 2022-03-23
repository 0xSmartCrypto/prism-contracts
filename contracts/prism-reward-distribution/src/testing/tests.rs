use std::str::FromStr;

use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Api, BankMsg, Coin, CosmosMsg, Decimal, OwnedDeps, Querier,
    StdError, Storage, SubMsg, Uint128, WasmMsg,
};

use cosmwasm_std::testing::{mock_env, mock_info};
use cw_asset::{Asset, AssetInfo};

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

const OWNER: &str = "owner";
const YASSET_TOKEN: &str = "ybeth";
const COLLECTOR: &str = "collector";
const DELEGATOR_REWARD_DENOM: &str = "uluna";

pub fn init<S: Storage, A: Api, Q: Querier>(deps: &mut OwnedDeps<S, A, Q>) {
    let msg = InstantiateMsg {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        collector: COLLECTOR.to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        whitelisted_assets: vec![AssetInfo::Native(DELEGATOR_REWARD_DENOM.to_string())],
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        yasset_staking: Some(YASSET_STAKING.to_string()),
        yasset_staking_x: Some(YASSET_STAKING_X.to_string()),
        protocol_fee: None,
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn test_initialization() {
    let mut deps = mock_dependencies(&[]);

    // invalid protocol fee
    let msg = InstantiateMsg {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        collector: COLLECTOR.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        protocol_fee: Decimal::from_ratio(11u128, 10u128),
        whitelisted_assets: vec![AssetInfo::Native(DELEGATOR_REWARD_DENOM.to_string())],
    };

    let info = mock_info(OWNER, &[]);
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::InvalidProtocolFee {});

    // valid init
    let msg = InstantiateMsg {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        collector: COLLECTOR.to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        whitelisted_assets: vec![AssetInfo::Native(DELEGATOR_REWARD_DENOM.to_string())],
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // verify config storage
    let state = QueryMsg::Config {};
    let config_response: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = ConfigResponse {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        collector: COLLECTOR.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        yasset_staking: "".to_string(),
        yasset_staking_x: "".to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        initialized: false,
    };
    assert_eq!(config_response, expected_result);

    // error - try to distribute rewards prior to initialization
    let msg = ExecuteMsg::DistributeRewards {};
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::NotInitialized {});

    //update config
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        yasset_staking: Some(YASSET_STAKING.to_string()),
        yasset_staking_x: Some(YASSET_STAKING_X.to_string()),
        protocol_fee: None,
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // verify config storage
    let state = QueryMsg::Config {};
    let config_response: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = ConfigResponse {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        collector: COLLECTOR.to_string(),
        yasset_token: YASSET_TOKEN.to_string(),
        yasset_staking: YASSET_STAKING.to_string(),
        yasset_staking_x: YASSET_STAKING_X.to_string(),
        protocol_fee: Decimal::from_ratio(1u128, 10u128),
        initialized: true,
    };
    assert_eq!(config_response, expected_result);
}

#[test]
fn test_distribute_rewards_native() {
    let mut deps = mock_dependencies(&[]);

    init(&mut deps);

    let reward_denom = "uluna";
    let reward_asset = Asset {
        info: AssetInfo::Native(reward_denom.to_string()),
        amount: Uint128::from(100u128),
    };

    // set contract balance=100 - this is the reward that will be distributed
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: reward_denom.to_string(),
            amount: Uint128::new(100u128),
        },
    )]);

    let msg = ExecuteMsg::DistributeRewards {};

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
                denom: reward_denom.to_string(),
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
                    denom: reward_denom.to_string(),
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
                    denom: reward_denom.to_string(),
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
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: COLLECTOR.to_string(),
                amount: vec![Coin {
                    denom: reward_denom.to_string(),
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
                    denom: reward_denom.to_string(),
                    amount: Uint128::from(27u128)
                }],
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
                funds: vec![Coin {
                    denom: reward_denom.to_string(),
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

    let reward_denom = "ANC";
    let reward_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked(reward_denom)),
        amount: Uint128::from(100u128),
    };

    // set contract balance=100 anc tokens - this is the reward that will be distributed
    deps.querier.with_token_balances(&[(
        &reward_denom.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &reward_asset.amount)],
    )]);

    let msg = ExecuteMsg::DistributeRewards {};

    // whitelist ANC
    let whitelist_msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("ANC".to_string())),
    };
    let info = mock_info(OWNER, &[]);
    execute(deps.as_mut(), mock_env(), info, whitelist_msg).unwrap();

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
            contract_addr: reward_denom.to_string(),
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
                contract_addr: reward_denom.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: COLLECTOR.to_string(),
                    amount: Uint128::from(73u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_denom.to_string(),
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
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_denom.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: COLLECTOR.to_string(),
                    amount: Uint128::from(28u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_denom.to_string(),
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
                contract_addr: reward_denom.to_string(),
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
            assets: vec![AssetInfo::Native("uluna".to_string())]
        }
    );

    // whitelist one more
    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("mir0000".to_string())),
    };

    // unauth attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // valid attempt
    let info = mock_info(OWNER, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist_reward_asset"),
            attr("whitelisted_asset", "cw20:mir0000"),
        ]
    );

    // try again, same symbol, duplicate whitelist asset error
    let info = mock_info(OWNER, &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::DuplicateWhitelistAsset {
            asset: "cw20:mir0000".to_string()
        }
    );

    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Native("uluna".to_string()),
                AssetInfo::Cw20(Addr::unchecked("mir0000".to_string())),
            ]
        }
    );

    // whitelist native asset
    let msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Native("uusd".to_string()),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist_reward_asset"),
            attr("whitelisted_asset", "native:uusd"),
        ]
    );

    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Native("uluna".to_string()),
                AssetInfo::Cw20(Addr::unchecked("mir0000".to_string())),
                AssetInfo::Native("uusd".to_string()),
            ]
        }
    );

    // distribute rewards for non-whitelisted asset
    let reward_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
        amount: Uint128::from(100u128),
    };

    // set contract balance=100 anc tokens - this is the reward that will be distributed
    deps.querier.with_token_balances(&[(
        &reward_asset.info.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &reward_asset.amount)],
    )]);
    // whitelist anc0000
    let whitelist_msg = ExecuteMsg::WhitelistRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("anc0000".to_string())),
    };
    let info = mock_info(OWNER, &[]);
    execute(deps.as_mut(), mock_env(), info, whitelist_msg).unwrap();

    // verify anc0000 added
    let res: RewardAssetWhitelistResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::RewardAssetWhitelist {}).unwrap())
            .unwrap();
    assert_eq!(
        res,
        RewardAssetWhitelistResponse {
            assets: vec![
                AssetInfo::Native("uluna".to_string()),
                AssetInfo::Cw20(Addr::unchecked("mir0000".to_string())),
                AssetInfo::Native("uusd".to_string()),
                AssetInfo::Cw20(Addr::unchecked("anc0000".to_string())),
            ]
        }
    );

    // add some vault balance to avoid empty vault error inside DistributeRewards
    deps.querier.with_vault_state(&Uint128::from(1000u128));

    // successful distribute rewards for anc0000
    let msg = ExecuteMsg::DistributeRewards {};
    let info = mock_info(VAULT, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // remove mir0000 from whitelist
    let msg = ExecuteMsg::RemoveRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("mir0000")),
    };

    // unauth attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // valid attempt
    let info = mock_info(OWNER, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "remove_whitelisted_reward_asset"),
            attr("removed_asset", "cw20:mir0000"),
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
                AssetInfo::Native("uluna".to_string()),
                AssetInfo::Native("uusd".to_string()),
                AssetInfo::Cw20(Addr::unchecked("anc0000".to_string())),
            ]
        }
    );

    // try to remove non whitelisted asset
    let msg = ExecuteMsg::RemoveRewardAsset {
        asset: AssetInfo::Cw20(Addr::unchecked("random0000")),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::RewardAssetNotWhitelisted {
            asset: "cw20:random0000".to_string()
        }
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // query config
    let state = QueryMsg::Config {};
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    assert_eq!(
        res,
        ConfigResponse {
            owner: OWNER.to_string(),
            vault: VAULT.to_string(),
            collector: COLLECTOR.to_string(),
            yasset_token: YASSET_TOKEN.to_string(),
            yasset_staking: YASSET_STAKING.to_string(),
            yasset_staking_x: YASSET_STAKING_X.to_string(),
            protocol_fee: Decimal::from_ratio(1u128, 10u128),
            initialized: true,
        }
    );

    // unauthorized
    let info = mock_info("not_the_owner_0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        yasset_staking: None,
        yasset_staking_x: None,
        protocol_fee: Some(Decimal::from_str("1.1").unwrap()),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // invalid protocol fee
    let info = mock_info(OWNER, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        yasset_staking: None,
        yasset_staking_x: None,
        protocol_fee: Some(Decimal::from_str("0.6").unwrap()),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidProtocolFee {});

    // invalid owner addr
    let info = mock_info(OWNER, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("ab".to_string()),
        yasset_staking: None,
        yasset_staking_x: None,
        protocol_fee: None,
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::Std(StdError::generic_err(
            "Invalid input: human address too short".to_string()
        ))
    );

    // success
    // unauthorized
    let info = mock_info(OWNER, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("new_owner_0000".to_string()),
        yasset_staking: None,
        yasset_staking_x: None,
        protocol_fee: Some(Decimal::from_str("0.4").unwrap()),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 0);
    assert_eq!(res.attributes, vec![attr("action", "update_config")]);

    // query config get verify changes
    let state = QueryMsg::Config {};
    let res: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    assert_eq!(
        res,
        ConfigResponse {
            owner: "new_owner_0000".to_string(),
            vault: VAULT.to_string(),
            collector: COLLECTOR.to_string(),
            yasset_token: YASSET_TOKEN.to_string(),
            yasset_staking: YASSET_STAKING.to_string(),
            yasset_staking_x: YASSET_STAKING_X.to_string(),
            protocol_fee: Decimal::from_str("0.4").unwrap(),
            initialized: true,
        }
    );
}
