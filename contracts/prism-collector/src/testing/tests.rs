use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, coin, from_binary, to_binary, Addr, Coin, CosmosMsg, MemoryStorage, OwnedDeps, StdError,
    SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_asset::{Asset, AssetInfo};
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use prismswap::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};

// helper to successfully init with reasonable defaults
pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        prismswap_factory: "prismfactory0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        base_denom: "uusd".to_string(),
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        prismswap_factory: "prismfactory0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();

    assert_eq!("astrofactory0000", config.astroport_factory.as_str());
    assert_eq!("gov0000", config.distribution_contract.as_str());
    assert_eq!("prismfactory0000", config.prismswap_factory.as_str());
    assert_eq!("prism0000", config.prism_token.as_str());
    assert_eq!("uusd", config.base_denom.as_str());
}

#[test]
fn test_convert_and_send() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("anc0000")),
        ],
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Native("uusd".to_string()),
        ],
    ]);

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
                amount: Uint128::from(200u128),
            },
        ],
        receiver: Some("user0000".to_string()),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::new(100u128),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "anc0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::new(200u128),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: Uint128::new(100u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some("user0000".to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "anc0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "anc0000uusd".to_string(),
                    amount: Uint128::new(200u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("user0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
}

#[test]
fn test_convert_and_send_native() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uluna_asset = Asset {
        info: AssetInfo::Native("uluna".to_string()),
        amount: Uint128::from(amount),
    };
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone()],
        receiver: Some("user0000".to_string()),
    };

    // failure - no funds sent
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::Std(StdError::generic_err(
            "Native token balance mismatch between the argument and the transferred"
        ))
    );

    // failure - wrong coin sent
    let info = mock_info("addr0000", &[coin(amount, "ukrt")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::Std(StdError::generic_err(
            "Native token balance mismatch between the argument and the transferred"
        ))
    );

    // failure - wrong amount sent
    let info = mock_info("addr0000", &[coin(amount + 1, "uluna")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::Std(StdError::generic_err(
            "Native token balance mismatch between the argument and the transferred"
        ))
    );

    // failure - missing route
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "native:uluna".to_string()
        }
    );

    // add pair for uluna/uusd
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uluna".to_string()),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    // success - since no pair exists from uluna to prism, perform a swap from
    // uluna to uusd and register a BaseSwapHook message.
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ulunauusd".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: uluna_asset.amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("user0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // success - same as above but with empty receiver
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone()],
        receiver: None,
    };
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ulunauusd".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: uluna_asset.amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: info.sender,
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);

    // success - this time add a pair for uluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook
    deps.querier.with_pairs(&[
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Native("prism0000".to_string()),
        ],
    ]);
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uluna".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone(),
                max_spread: None,
                belief_price: None,
                to: Some(info.sender.to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uluna".to_string(),
                amount: uluna_asset.amount,
            }],
        })),]
    );

    // success - convert and send two native coins with direct prism pairs
    deps.querier.with_pairs(&[
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Native("prism0000".to_string()),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Native("prism0000".to_string()),
        ],
    ]);
    let uusd_asset = Asset {
        info: AssetInfo::Native("uusd".to_string()),
        amount: Uint128::from(amount),
    };

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone(), uusd_asset.clone()],
        receiver: None,
    };

    let info = mock_info("addr0000", &[coin(amount, "uluna"), coin(amount, "uusd")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "prism0000uluna".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some(info.sender.to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: uluna_asset.amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "prism0000uusd".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uusd_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some(info.sender.to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: uusd_asset.amount,
                }],
            })),
        ]
    );
}

#[test]
fn test_convert_and_send_cw20() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone()],
        receiver: Some("user0000".to_string()),
    };

    // failure - missing route
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "cw20:yluna0000".to_string()
        }
    );

    // add pair for yluna0000/uusd
    deps.querier.with_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    // success - since no pair exists from yluna to prism, perform a swap from
    // yluna to uusd and register a BaseSwapHook message.
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "uusdyluna0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("user0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);

    // success - same as above but with empty receiver
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone()],
        receiver: None,
    };
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "uusdyluna0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: info.sender,
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // success - this time add a pair for yluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(info.sender.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    // success - convert and send two cw20 tokens with direct prism pairs
    let pluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("pluna0000")),
        amount: Uint128::from(amount),
    };

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset, pluna_asset],
        receiver: None,
    };
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("pluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "pluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(info.sender.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "pluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "pluna0000prism0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(info.sender.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_convert_and_send_native_and_cw20() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uluna_asset = Asset {
        info: AssetInfo::Native("uluna".to_string()),
        amount: Uint128::from(amount),
    };
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone(), yluna_asset],
        receiver: None,
    };

    // add direct prism pairs for uluna and yluna
    deps.querier.with_pairs(&[
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "prism0000uluna".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some(info.sender.to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: uluna_asset.amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(info.sender.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);
}

#[test]
fn test_convert_and_send_astroport() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset],
        receiver: Some("user0000".to_string()),
    };

    // failure - missing route
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "cw20:yluna0000".to_string()
        }
    );

    // add astroport pair for yluna0000/uusd
    deps.querier.with_astro_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    // success - since no pair exists from yluna to prism, perform a swap from
    // yluna to uusd and register a BaseSwapHook message.
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::from(amount),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "uusdyluna0000".to_string(),
                    amount: Uint128::from(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string())
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("user0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);
}

#[test]
fn test_distribute() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    deps.querier.with_token_balances(&[
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100u128))],
        ),
        (
            &"anc0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(200u128))],
        ),
    ]);
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("anc0000")),
        ],
    ]);

    let asset_infos = vec![
        AssetInfo::Native("uluna".to_string()),
        AssetInfo::Cw20(Addr::unchecked("anc0000")),
    ];

    let msg = ExecuteMsg::Distribute { asset_infos };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "anc0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "anc0000uusd".to_string(),
                    amount: Uint128::new(200u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("gov0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ],
    );

    let asset_infos = vec![
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Cw20(Addr::unchecked("anc0000")),
        AssetInfo::Cw20(Addr::unchecked("pluna0000")),
    ];
    let msg = ExecuteMsg::Distribute { asset_infos };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: Uint128::new(100u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some("gov0000".to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "anc0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "anc0000uusd".to_string(),
                    amount: Uint128::new(200u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("gov0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
}

#[test]
fn test_distribute_native() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uluna_asset = Asset {
        info: AssetInfo::Native("uluna".to_string()),
        amount: Uint128::from(amount),
    };

    let asset_infos = vec![uluna_asset.info.clone()];
    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);

    // no uluna balance in contract, no messages emitted
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.messages, []);

    // add some uluna to the contract
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(amount),
        },
    )]);

    // failure - missing route
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "native:uluna".to_string()
        }
    );

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uluna".to_string()),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ulunauusd".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: uluna_asset.amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("gov0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            }))
        ],
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);

    // success - this time add a pair for uluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook, swap
    // recipient set to gov
    deps.querier.with_pairs(&[
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Native("prism0000".to_string()),
        ],
    ]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uluna".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone(),
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uluna".to_string(),
                amount: uluna_asset.amount,
            }],
        })),]
    );
}

#[test]
fn test_distribute_cw20() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };

    let asset_infos = vec![yluna_asset.info];
    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);

    // no yluna balance in contract, no messages emitted
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.messages, []);

    // add some yluna to the contract
    deps.querier.with_token_balances(&[(
        &"yluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(amount))],
    )]);

    // failure - missing route
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "cw20:yluna0000".to_string()
        }
    );

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "uusdyluna0000".to_string(),
                    amount: Uint128::new(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("gov0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ],
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);

    // success - this time add a pair for yluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook, swap
    // recipient set to gov
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000yluna0000".to_string(),
                amount: Uint128::new(amount),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some("gov0000".to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );
}

#[test]
fn test_distribute_native_and_cw20() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uluna_asset = Asset {
        info: AssetInfo::Native("uluna".to_string()),
        amount: Uint128::from(amount),
    };
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };

    // add direct prism pairs for uluna and yluna
    deps.querier.with_pairs(&[
        [
            AssetInfo::Native("uluna".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    let asset_infos = vec![uluna_asset.info.clone(), yluna_asset.info];
    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);

    // no balance in contract, no messages emitted
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(res.messages, []);

    // add uluna balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(amount),
        },
    )]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uluna".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone(),
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uluna".to_string(),
                amount: uluna_asset.amount,
            }],
        })),]
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);

    // remove uluna balance, add yluna balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::zero(),
        },
    )]);
    deps.querier.with_token_balances(&[(
        &"yluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(amount))],
    )]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000yluna0000".to_string(),
                amount: Uint128::new(amount),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some("gov0000".to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    // add balances for both uluna and yluna
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uluna".to_string(),
            amount: Uint128::new(amount),
        },
    )]);
    deps.querier.with_token_balances(&[(
        &"yluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(amount))],
    )]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "prism0000uluna".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some("gov0000".to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: uluna_asset.amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: Uint128::new(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some("gov0000".to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );
}

#[test]
fn test_distribute_astroport() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };

    let asset_infos = vec![yluna_asset.info];
    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);

    // add some yluna to the contract
    deps.querier.with_token_balances(&[(
        &"yluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(amount))],
    )]);

    // failure - missing route
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "cw20:yluna0000".to_string()
        }
    );

    // add astroport pair
    deps.querier.with_astro_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "uusdyluna0000".to_string(),
                    amount: Uint128::new(amount),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("gov0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ],
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);
}

#[test]
fn test_base_swap_hook() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::BaseSwapHook {
        receiver: Addr::unchecked("gov0000"),
        prev_base_balance: Uint128::zero(),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // no balance - successful return but no messages generated
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(res.messages, vec![]);

    // add some uusd balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(11134u128),
        },
    )]);

    // missing route
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: "native:uusd".to_string()
        }
    );

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uusd".to_string()),
        AssetInfo::Cw20(Addr::unchecked("prism0000")),
    ]]);

    // success
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "base_swap_hook")]);
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(11134u128),
                    info: AssetInfo::Native("uusd".to_string()),
                },
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(11134u128),
            }],
        }))]
    );

    // success with a receiver specified
    let receiver = Addr::unchecked("addr0001");
    let msg = ExecuteMsg::BaseSwapHook {
        receiver: receiver.clone(),
        prev_base_balance: Uint128::zero(),
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "base_swap_hook")]);
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(11134u128),
                    info: AssetInfo::Native("uusd".to_string()),
                },
                max_spread: None,
                belief_price: None,
                to: Some(receiver.to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(11134u128),
            }],
        }))]
    );
}

#[test]
fn test_distribute_with_existing_prev_base_balance() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    // create Distribute message with yluna0000 and uusd
    let yluna_asset_info = AssetInfo::Cw20(Addr::unchecked("yluna0000"));
    let uusd_asset_info = AssetInfo::Native("uusd".to_string());
    let asset_infos = vec![yluna_asset_info.clone(), uusd_asset_info.clone()];
    let yluna_asset = Asset {
        info: yluna_asset_info,
        amount: Uint128::from(1000u128),
    };
    let uusd_asset = Asset {
        info: uusd_asset_info,
        amount: Uint128::from(2000u128),
    };

    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);

    // add 1000 yluna to the contract balance
    deps.querier.with_token_balances(&[(
        &"yluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &yluna_asset.amount)],
    )]);

    // add 2000 uusd to the contract balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: uusd_asset.amount,
        },
    )]);

    // add yluna/uusd and prism/uusd pairs
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    // distribute with yluna balance=1000, uusd balance=2000
    // in this hypothetical case, there's no yluna/prism pair, so we need
    // a base swap hook using 0 as the prev_base_balance so that the hook
    // will convert our entire uusd balance to prism
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "uusdyluna0000".to_string(),
                    amount: yluna_asset.amount,
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(MOCK_CONTRACT_ADDR.to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::BaseSwapHook {
                    receiver: Addr::unchecked("gov0000"),
                    prev_base_balance: Uint128::zero(),
                })
                .unwrap(),
                funds: vec![],
            })),
        ],
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);

    // run the same test as above, but this time add a direct route from
    // yluna->prism.  Therefore, there's no need for the base swap hook since
    // we can convert both yluna and uusd directly to prism.  Swap receivers
    // both set to gov in this case
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "prism0000yluna0000".to_string(),
                    amount: yluna_asset.amount,
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some("gov0000".to_string()),
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "prism0000uusd".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uusd_asset.clone(),
                    max_spread: None,
                    belief_price: None,
                    to: Some("gov0000".to_string()),
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: uusd_asset.amount,
                }],
            })),
        ],
    );
}

#[test]
fn test_base_swap_hook_with_prev_base_balance() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::BaseSwapHook {
        receiver: Addr::unchecked("gov0000"),
        prev_base_balance: Uint128::from(5000u128),
    };

    // add some uusd balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(11134u128),
        },
    )]);

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uusd".to_string()),
        AssetInfo::Cw20(Addr::unchecked("prism0000")),
    ]]);

    // success
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "base_swap_hook")]);
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(6134u128),
                    info: AssetInfo::Native("uusd".to_string()),
                },
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(6134u128),
            }],
        }))]
    );

    // same thing, but zero out the prev_base_balance, this will result in entire
    // 11134 getting swapped and sent to gov
    let msg = ExecuteMsg::BaseSwapHook {
        receiver: Addr::unchecked("gov0000"),
        prev_base_balance: Uint128::zero(),
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "base_swap_hook")]);
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(11134u128),
                    info: AssetInfo::Native("uusd".to_string()),
                },
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(11134u128),
            }],
        }))]
    );
}

#[test]
fn test_duplicate_assets() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let yluna_asset_info = AssetInfo::Cw20(Addr::unchecked("yluna0000"));
    let uusd_asset_info = AssetInfo::Native("uusd".to_string());
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("yluna0000"));
    let yluna_asset = Asset {
        info: yluna_asset_info.clone(),
        amount: Uint128::from(1000u128),
    };
    let uusd_asset = Asset {
        info: uusd_asset_info.clone(),
        amount: Uint128::from(2000u128),
    };
    let prism_asset = Asset {
        info: prism_asset_info.clone(),
        amount: Uint128::from(3000u128),
    };

    // add direct routes
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    // CompareAndSend duplicate assets
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone(), uusd_asset.clone(), prism_asset],
        receiver: None,
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicateAssets {});

    // add 1000 yluna to the contract balance
    deps.querier.with_token_balances(&[(
        &"yluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &yluna_asset.amount)],
    )]);

    // add 2000 uusd to the contract balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: uusd_asset.amount,
        },
    )]);

    // Distribute duplicate assets
    let msg = ExecuteMsg::Distribute {
        asset_infos: vec![yluna_asset_info, uusd_asset_info, prism_asset_info],
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicateAssets {});
}

#[test]
fn test_convert_and_send_with_prism_and_uusd() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let yluna_asset_info = AssetInfo::Cw20(Addr::unchecked("yluna0000"));
    let uusd_asset_info = AssetInfo::Native("uusd".to_string());
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let yluna_asset = Asset {
        info: yluna_asset_info,
        amount: Uint128::from(1000u128),
    };
    let uusd_asset = Asset {
        info: uusd_asset_info,
        amount: Uint128::from(2000u128),
    };
    let prism_asset = Asset {
        info: prism_asset_info,
        amount: Uint128::from(3000u128),
    };

    // add direct routes
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    // set uusd contract balance to 2000 so that the contract sees the funds sent
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(2000u128),
        },
    )]);

    // ConvertAndSend with yluna, uusd, and prism, direct routes
    // this results in the following:
    // - TransferFrom for yluna to MOCK_CONTRACT_ADDR
    // - TransferFrom for prism to MOCK_CONTRACT_ADDR
    // - Swap yluna -> prism, send to receiver
    // - Transfer prism to receiver
    // - Swap uusd -> prism, send to receiver (uusd swaps are last)
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone(), uusd_asset.clone(), prism_asset.clone()],
        receiver: None,
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let recipient = info.sender.to_string();
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 5);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(1000u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(3000u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000yluna0000".to_string(),
                amount: Uint128::new(1000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some(recipient.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(3000u128),
                recipient: recipient.to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[4],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(2000u128),
                    info: AssetInfo::Native("uusd".to_string()),
                },
                max_spread: None,
                belief_price: None,
                to: Some(recipient),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(2000u128),
            }],
        }))
    );

    // change routes, nothing direct from yluna to prism
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    // ConvertAndSend with yluna, uusd, and prism, indirect route for yluna
    // this results in the following:
    // - TransferFrom for yluna to MOCK_CONTRACT_ADDR
    // - TransferFrom for prism to MOCK_CONTRACT_ADDR
    // - Swap yluna -> uusd, send to MOCK_CONTRACT_ADDR, register hook
    // - Transfer prism to receiver
    // - Base swap hook, recipient = reciever, no prev balance exists
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone(), uusd_asset.clone(), prism_asset.clone()],
        receiver: None,
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let recipient = info.sender.to_string();
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 5);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(1000u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(3000u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "uusdyluna0000".to_string(),
                amount: Uint128::new(1000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(3000u128),
                recipient: recipient.to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[4],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: Addr::unchecked(recipient),
                prev_base_balance: Uint128::zero(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    // change uusd contract balance from 2000 to 5000.  this will result
    // in the contract using a prev balance of 3000 for the base swap hook
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(5000u128),
        },
    )]);

    // same as prior, but with a uusd starting balance of 5000 to test
    // the prev_balance on the swap hook
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset, uusd_asset, prism_asset],
        receiver: None,
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let recipient = info.sender.to_string();
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 5);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(1000u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(3000u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "uusdyluna0000".to_string(),
                amount: Uint128::new(1000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(3000u128),
                recipient: recipient.to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[4],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: Addr::unchecked(recipient),
                prev_base_balance: Uint128::from(3000u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_distribute_with_prism_and_uusd() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let yluna_asset_info = AssetInfo::Cw20(Addr::unchecked("yluna0000"));
    let uusd_asset_info = AssetInfo::Native("uusd".to_string());
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let yluna_asset = Asset {
        info: yluna_asset_info.clone(),
        amount: Uint128::from(1000u128),
    };
    let uusd_asset = Asset {
        info: uusd_asset_info.clone(),
        amount: Uint128::from(2000u128),
    };
    let prism_asset = Asset {
        info: prism_asset_info.clone(),
        amount: Uint128::from(3000u128),
    };

    // add direct routes
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    // set contract balances of 1000 yluna, 2000 uusd, and 3000 prism
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: uusd_asset.amount,
        },
    )]);
    deps.querier.with_token_balances(&[
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &yluna_asset.amount)],
        ),
        (
            &"prism0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &prism_asset.amount)],
        ),
    ]);

    // Distribute with yluna, uusd, and prism, direct routes
    // this results in the following:
    // - Swap yluna -> prism, send to receiver
    // - Transfer prism to receiver
    // - Swap uusd -> prism, send to receiver (uusd swaps are last)
    let msg = ExecuteMsg::Distribute {
        asset_infos: vec![
            yluna_asset_info.clone(),
            uusd_asset_info.clone(),
            prism_asset_info.clone(),
        ],
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let recipient = "gov0000";
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000yluna0000".to_string(),
                amount: Uint128::new(1000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some(recipient.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(3000u128),
                recipient: recipient.to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(2000u128),
                    info: AssetInfo::Native("uusd".to_string()),
                },
                max_spread: None,
                belief_price: None,
                to: Some(recipient.to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(2000u128),
            }],
        }))
    );

    // change routes, nothing direct from yluna to prism
    deps.querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
        ],
    ]);

    // Distribute with yluna, uusd, and prism, indirect route for yluna
    // this results in the following:
    // - Swap yluna -> uusd, send to MOCK_CONTRACT_ADDR
    // - Transfer prism to receiver
    // - Base swap hook, recipient = reciever, prev_base_balance set to zero
    //   so that the entire balance is transferred
    let msg = ExecuteMsg::Distribute {
        asset_infos: vec![yluna_asset_info, uusd_asset_info, prism_asset_info],
    };
    let info = mock_info("addr0000", &[coin(2000, "uusd")]);
    let recipient = "gov0000";
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "uusdyluna0000".to_string(),
                amount: Uint128::new(1000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: None,
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(3000u128),
                recipient: recipient.to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: Addr::unchecked(recipient.to_string()),
                prev_base_balance: Uint128::zero(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_asset_with_zero_amount() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 0u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone()],
        receiver: Some("user0000".to_string()),
    };

    // add route for yluna0000/uusd
    deps.querier.with_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Native("uusd".to_string()),
    ]]);

    // zero amount, no messages
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages, vec![]);

    // same thing with Distribute, we don't have any yluna balance
    let msg = ExecuteMsg::Distribute {
        asset_infos: vec![yluna_asset.info],
    };
    // zero amount, no messages
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages, vec![]);
}
