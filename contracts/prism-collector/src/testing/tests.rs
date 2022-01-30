use crate::contract::{execute, instantiate, query};
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, coin, from_binary, to_binary, Addr, Coin, CosmosMsg, MemoryStorage, OwnedDeps, StdError,
    SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_asset::{Asset, AssetInfo};
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use prismswap::asset::AssetInfo as PSAssetInfo;
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

    deps.querier.with_pairs(&[[
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        ],
        [
            AssetInfo::Native("uusd".to_string()).into(),
            AssetInfo::Cw20(Addr::unchecked("anc0000")).into(),
        ],
        [
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Native("uusd".to_string()).into(),
        ]]);

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
                amount: Uint128::from(100u128),
            }
            .into(),
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
                amount: Uint128::from(200u128),
            }
            .into(),
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
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    amount: Uint128::new(200u128),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
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
                    receiver: Some("user0000".to_string()),
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
        assets: vec![uluna_asset.clone().into()],
        receiver: Some("user0000".to_string()),
    };

    // failure - no funds sent
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("Missing funds payment: uluna"));

    // failure - wrong coin sent
    let info = mock_info("addr0000", &[coin(amount, "ukrt")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("Missing funds payment: uluna"));

    // failure - wrong amount sent
    let info = mock_info("addr0000", &[coin(amount + 1, "uluna")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Invalid uluna payment - funds/asset amount mismatch")
    );

    // failure - missing route
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("Missing route for native:uluna"));

    // add pair for uluna/uusd
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uluna".to_string()).into(),
        AssetInfo::Native("uusd".to_string()).into(),
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
                    offer_asset: uluna_asset.clone().into(),
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
                    receiver: Some("user0000".to_string()),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // success - same as above but with empty receiver
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone().into()],
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
                    offer_asset: uluna_asset.clone().into(),
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
                    receiver: Some(info.sender.to_string()),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);

    // success - this time add a pair for uluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook
    deps.querier.with_pairs(&[[
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Native("uusd".to_string()).into(),
        ],
        [
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Native("prism0000".to_string()).into(),
        ]]);
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uluna".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone().into(),
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
    deps.querier.with_pairs(&[[
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Native("prism0000".to_string()).into(),
        ],
        [
            AssetInfo::Native("uusd".to_string()).into(),
            AssetInfo::Native("prism0000".to_string()).into(),
        ]]);
    let uusd_asset = Asset {
        info: AssetInfo::Native("uusd".to_string()),
        amount: Uint128::from(amount),
    };

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone().into(), uusd_asset.clone().into()],
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
                    offer_asset: uluna_asset.clone().into(),
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
                    offer_asset: uusd_asset.clone().into(),
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
        assets: vec![yluna_asset.clone().into()],
        receiver: Some("user0000".to_string()),
    };

    // failure - missing route
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Missing route for cw20:yluna0000")
    );

    // add pair for yluna0000/uusd
    deps.querier.with_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        AssetInfo::Native("uusd".to_string()).into(),
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
                    receiver: Some("user0000".to_string()),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);

    // success - same as above but with empty receiver
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone().into()],
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
                    receiver: Some(info.sender.to_string()),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // success - this time add a pair for yluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook
    deps.querier.with_pairs(&[[
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Native("uusd".to_string()).into(),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ]]);
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
        assets: vec![yluna_asset.into(), pluna_asset.into()],
        receiver: None,
    };
    deps.querier.with_pairs(&[[
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("pluna0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ]]);
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
        assets: vec![uluna_asset.clone().into(), yluna_asset.into()],
        receiver: None,
    };

    // add direct prism pairs for uluna and yluna
    deps.querier.with_pairs(&[[
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ]]);

    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "prism0000uluna".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone().into(),
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
        assets: vec![yluna_asset.into()],
        receiver: Some("user0000".to_string()),
    };

    // failure - missing route
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Missing route for cw20:yluna0000")
    );

    // add astroport pair for yluna0000/uusd
    deps.querier.with_astro_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        AssetInfo::Native("uusd".to_string()).into(),
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
                    receiver: Some("user0000".to_string()),
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
    deps.querier.with_pairs(&[[
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        ],
        [
            AssetInfo::Native("uusd".to_string()).into(),
            AssetInfo::Cw20(Addr::unchecked("anc0000")).into(),
        ]]);

    let asset_infos: Vec<PSAssetInfo> = vec![
        AssetInfo::Native("uluna".to_string()).into(),
        AssetInfo::Cw20(Addr::unchecked("anc0000")).into(),
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
                msg: to_binary(&ExecuteMsg::BaseSwapHook { receiver: None }).unwrap(),
                funds: vec![],
            })),
        ],
    );

    let asset_infos: Vec<PSAssetInfo> = vec![
        AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        AssetInfo::Cw20(Addr::unchecked("anc0000")).into(),
        AssetInfo::Cw20(Addr::unchecked("pluna0000")).into(),
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
                msg: to_binary(&ExecuteMsg::BaseSwapHook { receiver: None }).unwrap(),
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

    let asset_infos: Vec<PSAssetInfo> = vec![uluna_asset.info.clone().into()];
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
    assert_eq!(err, StdError::generic_err("Missing route for native:uluna"));

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uluna".to_string()).into(),
        AssetInfo::Native("uusd".to_string()).into(),
    ]]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "ulunauusd".to_string(),
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: uluna_asset.clone().into(),
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
                msg: to_binary(&ExecuteMsg::BaseSwapHook { receiver: None }).unwrap(),
                funds: vec![],
            }))
        ],
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);

    // success - this time add a pair for uluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook, swap
    // recipient set to gov
    deps.querier.with_pairs(&[[
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Native("uusd".to_string()).into(),
        ],
        [
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Native("prism0000".to_string()).into(),
        ]]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uluna".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone().into(),
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

    let asset_infos: Vec<PSAssetInfo> = vec![yluna_asset.info.into()];
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
        StdError::generic_err("Missing route for cw20:yluna0000")
    );

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        AssetInfo::Native("uusd".to_string()).into(),
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
                msg: to_binary(&ExecuteMsg::BaseSwapHook { receiver: None }).unwrap(),
                funds: vec![],
            })),
        ],
    );
    assert_eq!(res.attributes, vec![attr("action", "distribute")]);

    // success - this time add a pair for yluna/prism as well so that
    // we do a direct swap and no longer need the BaseSwapHook, swap
    // recipient set to gov
    deps.querier.with_pairs(&[[
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Native("uusd".to_string()).into(),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ]]);
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
    deps.querier.with_pairs(&[[
            AssetInfo::Native("uluna".to_string()).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
            AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
        ]]);

    let asset_infos: Vec<PSAssetInfo> =
        vec![uluna_asset.info.clone().into(), yluna_asset.info.into()];
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
                offer_asset: uluna_asset.clone().into(),
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
                    offer_asset: uluna_asset.clone().into(),
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

    let asset_infos: Vec<PSAssetInfo> = vec![yluna_asset.info.into()];
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
        StdError::generic_err("Missing route for cw20:yluna0000")
    );

    // add astroport pair
    deps.querier.with_astro_pairs(&[[
        AssetInfo::Cw20(Addr::unchecked("yluna0000")).into(),
        AssetInfo::Native("uusd".to_string()).into(),
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
                msg: to_binary(&ExecuteMsg::BaseSwapHook { receiver: None }).unwrap(),
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

    let msg = ExecuteMsg::BaseSwapHook { receiver: None };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

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
    assert_eq!(err, StdError::generic_err("Missing route for native:uusd"));

    // add pair
    deps.querier.with_pairs(&[[
        AssetInfo::Native("uusd".to_string()).into(),
        AssetInfo::Cw20(Addr::unchecked("prism0000")).into(),
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
                }
                .into(),
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()), // by default sends to gov
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(11134u128),
            }],
        }))]
    );
}
