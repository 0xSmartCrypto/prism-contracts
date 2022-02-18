use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use astroport::pair::{Cw20HookMsg as AstroPairCw20HookMsg, ExecuteMsg as AstroPairExecuteMsg};
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, coin, from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, MemoryStorage, OwnedDeps,
    StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_asset::{Asset, AssetInfo};
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use prismswap::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};
use prismswap::router::{
    Cw20HookMsg as RouterCw20HookMsg, ExecuteMsg as RouterExecuteMsg, SwapOperation,
};
use std::str::FromStr;

// helper to successfully init with reasonable defaults
pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let info = mock_info("addr0000", &[]);
    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        prismswap_factory: "prismfactory0000".to_string(),
        prismswap_router: "prismrouter0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        base_denom: "uusd".to_string(),
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

pub fn astro_max_spread() -> Decimal {
    Decimal::from_str(astroport::pair::MAX_ALLOWED_SLIPPAGE).unwrap()
}

pub fn configure_default_pairs(querier: &mut WasmMockQuerier) {
    querier.with_astro_pairs(&[
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Cw20(Addr::unchecked("anc0000")),
        ],
        [
            AssetInfo::Native("uusd".to_string()),
            AssetInfo::Native("uluna".to_string()),
        ],
    ]);
    querier.with_pairs(&[
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Cw20(Addr::unchecked("xprism0000")),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Cw20(Addr::unchecked("cluna0000")),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        ],
        [
            AssetInfo::Cw20(Addr::unchecked("prism0000")),
            AssetInfo::Cw20(Addr::unchecked("pluna0000")),
        ],
    ]);
}

pub fn configure_default_balances(querier: &mut WasmMockQuerier) {
    querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(1000u128),
        },
    )]);

    querier.with_token_balances(&[
        (
            &"prism0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000u128))],
        ),
        (
            &"xprism0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(2000u128))],
        ),
        (
            &"cluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(3000u128))],
        ),
        (
            &"yluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(4000u128))],
        ),
        (
            &"pluna0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(5000u128))],
        ),
    ]);
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        prismswap_factory: "prismfactory0000".to_string(),
        prismswap_router: "prismrouter0000".to_string(),
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
    assert_eq!("prismrouter0000", config.prismswap_router.as_str());
    assert_eq!("prism0000", config.prism_token.as_str());
    assert_eq!("uusd", config.base_denom.as_str());
}

#[test]
pub fn test_convert_and_send_failures_uusd() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uusd_asset = Asset {
        info: AssetInfo::Native("uusd".to_string()),
        amount: Uint128::from(amount),
    };
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uusd_asset.clone()],
        receiver: Some("user0000".to_string()),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
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
    let info = mock_info("addr0000", &[coin(amount + 1, "uusd")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::Std(StdError::generic_err(
            "Native token balance mismatch between the argument and the transferred"
        ))
    );

    // failure - missing route
    let info = mock_info("addr0000", &[coin(amount, "uusd")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: uusd_asset.info.clone(),
            dest_asset: AssetInfo::Cw20(Addr::unchecked("prism0000")),
        }
    );

    // failure - duplicate assets
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uusd_asset.clone(), uusd_asset],
        receiver: Some("user0000".to_string()),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };
    let info = mock_info("addr0000", &[coin(amount, "uusd")]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicateAssets {});
}

#[test]
pub fn test_convert_and_send_failures_yluna() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000".to_string())),
        amount: Uint128::from(amount),
    };
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone()],
        receiver: Some("user0000".to_string()),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };

    // failure - missing route
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: yluna_asset.info.clone(),
            dest_asset: AssetInfo::Cw20(Addr::unchecked("prism0000")),
        }
    );

    // failure - duplicate assets
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone(), yluna_asset],
        receiver: Some("user0000".to_string()),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicateAssets {});
}

#[test]
fn test_convert_and_send_uusd() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uusd_asset = Asset {
        info: AssetInfo::Native("uusd".to_string()),
        amount: Uint128::from(amount),
    };

    configure_default_pairs(&mut deps.querier);

    // ConvertAndSend uusd -> prism, direct prismswap route
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uusd_asset.clone()],
        receiver: None,
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };
    let info = mock_info("addr0000", &[coin(amount, "uusd")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
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
    );

    // ConvertAndSend uusd -> prism, direct prismswap route, receiver set
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uusd_asset.clone()],
        receiver: Some("user0000".to_string()),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };
    let info = mock_info("addr0000", &[coin(amount, "uusd")]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: uusd_asset.clone(),
                max_spread: None,
                belief_price: None,
                to: Some("user0000".to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: uusd_asset.amount,
            }],
        })),
    );

    // ConvertAndSend uusd -> pluna, requires prismswap router through prism
    let dest_asset_info = AssetInfo::Cw20(Addr::unchecked("pluna0000"));
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uusd_asset.clone()],
        receiver: None,
        dest_asset_info: dest_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[coin(amount, "uusd")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(res.attributes, vec![attr("action", "convert_and_send")]);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prismrouter0000".to_string(),
            msg: to_binary(&RouterExecuteMsg::ExecuteSwapOperations {
                operations: vec![
                    SwapOperation::PrismSwap {
                        offer_asset_info: uusd_asset.info.clone(),
                        ask_asset_info: prism_asset_info.clone(),
                    },
                    SwapOperation::PrismSwap {
                        offer_asset_info: prism_asset_info,
                        ask_asset_info: dest_asset_info,
                    },
                ],
                minimum_receive: None,
                to: Some(info.sender),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: uusd_asset.amount,
            }],
        })),
    );
}

#[test]
fn test_convert_and_send_yluna() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount),
    };
    configure_default_pairs(&mut deps.querier);

    // ConvertAndSend yluna -> prism, direct prismswap route
    // 1 - TransferFrom yluna to get funds
    // 2 - prismswap swap yluna -> prism
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone()],
        receiver: None,
        dest_asset_info: prism_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages[0],
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
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000yluna0000".to_string(),
                amount: Uint128::new(100u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: Some(info.sender.to_string()),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    // ConvertAndSend yluna -> pluna, uses prismswap router
    // is now pluna
    // 1 - TransferFrom yLuna
    // 2 - prismswap router swap (yluna -> prism -> pLuna)
    let pluna_asset_info = AssetInfo::Cw20(Addr::unchecked("pluna0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone()],
        receiver: None,
        dest_asset_info: pluna_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages[0],
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
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prismrouter0000".to_string(),
                amount: Uint128::from(amount),
                msg: to_binary(&RouterCw20HookMsg::ExecuteSwapOperations {
                    operations: vec![
                        SwapOperation::PrismSwap {
                            offer_asset_info: yluna_asset.info,
                            ask_asset_info: prism_asset_info.clone(),
                        },
                        SwapOperation::PrismSwap {
                            offer_asset_info: prism_asset_info,
                            ask_asset_info: pluna_asset_info,
                        },
                    ],
                    minimum_receive: None,
                    to: Some(info.sender),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    )
}

#[test]
fn test_convert_and_send_anc() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let anc_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
        amount: Uint128::from(amount),
    };
    configure_default_pairs(&mut deps.querier);

    // ConvertAndSend anc -> prism, requires
    // 1 - TransferFrom anc to get funds
    // 2 - astroport anc->uusd swap
    // 3 - base hook to convert to prism
    let dest_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![anc_asset.clone()],
        receiver: None,
        dest_asset_info: dest_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(100u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anc0000uusd".to_string(),
                amount: Uint128::new(100u128),
                msg: to_binary(&AstroPairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: Some(astro_max_spread()),
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
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
                receiver: info.sender,
                prev_base_balance: Uint128::zero(),
                dest_asset_info,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    // ConvertAndSend anc -> pluna, same as above but base hook dest_asset_info
    // is now pluna
    // 1 - TransferFrom anc
    // 2 - astroport anc -> uusd swap
    // 3 - base hook to convert to pluna
    let dest_asset_info = AssetInfo::Cw20(Addr::unchecked("pluna0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![anc_asset],
        receiver: None,
        dest_asset_info: dest_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(100u128),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anc0000uusd".to_string(),
                amount: Uint128::new(100u128),
                msg: to_binary(&AstroPairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: Some(astro_max_spread()),
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
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
                receiver: info.sender,
                prev_base_balance: Uint128::zero(),
                dest_asset_info,
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_convert_and_send_luna() {
    // we don't currently need luna conversion, but testing anyway to verify
    // native asset swaps through astroport, since it should work
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount = 100u128;
    let uluna_asset = Asset {
        info: AssetInfo::Native("uluna".to_string()),
        amount: Uint128::from(amount),
    };
    configure_default_pairs(&mut deps.querier);

    // ConvertAndSend uluna -> prism, requires
    // 2 - astroport uluna->uusd swap
    // 3 - base hook to convert to prism
    let dest_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone()],
        receiver: None,
        dest_asset_info: dest_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "ulunauusd".to_string(),
            msg: to_binary(&AstroPairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone().into(),
                max_spread: Some(astro_max_spread()),
                belief_price: None,
                to: Some(MOCK_CONTRACT_ADDR.to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uluna".to_string(),
                amount: uluna_asset.amount,
            }],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: info.sender,
                prev_base_balance: Uint128::zero(),
                dest_asset_info,
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    // ConvertAndSend luna -> pluna, same as above but base hook dest_asset_info
    // is now pluna
    // 2 - astroport uluna->uusd swap
    // 3 - base hook to convert to pluna
    let dest_asset_info = AssetInfo::Cw20(Addr::unchecked("pluna0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![uluna_asset.clone()],
        receiver: None,
        dest_asset_info: dest_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[coin(amount, "uluna")]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "ulunauusd".to_string(),
            msg: to_binary(&AstroPairExecuteMsg::Swap {
                offer_asset: uluna_asset.clone().into(),
                max_spread: Some(astro_max_spread()),
                belief_price: None,
                to: Some(MOCK_CONTRACT_ADDR.to_string()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uluna".to_string(),
                amount: uluna_asset.amount,
            }],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::BaseSwapHook {
                receiver: info.sender,
                prev_base_balance: Uint128::zero(),
                dest_asset_info,
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_convert_and_send_yluna_and_anc() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount1 = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount1),
    };
    let amount2 = 200u128;
    let anc_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
        amount: Uint128::from(amount2),
    };
    configure_default_pairs(&mut deps.querier);

    // ConvertAndSend (yluna, anc)-> prism:
    // 1 - TransferFrom yluna
    // 2 - TransferFrom anc
    // 3 - prismswap swap yluna -> prism
    // 4 - astroport swap anc -> uusd
    // 5 - base hook
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone(), anc_asset.clone()],
        receiver: None,
        dest_asset_info: prism_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 5);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(amount1),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(amount2),
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
                amount: Uint128::new(amount1),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: Some(info.sender.to_string()),
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
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anc0000uusd".to_string(),
                amount: Uint128::new(amount2),
                msg: to_binary(&AstroPairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: Some(astro_max_spread()),
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
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
                receiver: info.sender,
                prev_base_balance: Uint128::zero(),
                dest_asset_info: prism_asset_info.clone(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    // ConvertAndSend (yluna, anc)-> pluna:
    // 1 - TransferFrom yluna
    // 2 - TransferFrom anc
    // 3 - prismswap router swap yluna -> prism -> pluna
    // 4 - astroport swap anc -> uusd
    // 5 - base hook
    let pluna_asset_info = AssetInfo::Cw20(Addr::unchecked("pluna0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset.clone(), anc_asset],
        receiver: None,
        dest_asset_info: pluna_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 5);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(amount1),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(amount2),
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
                contract: "prismrouter0000".to_string(),
                amount: Uint128::from(amount1),
                msg: to_binary(&RouterCw20HookMsg::ExecuteSwapOperations {
                    operations: vec![
                        SwapOperation::PrismSwap {
                            offer_asset_info: yluna_asset.info,
                            ask_asset_info: prism_asset_info.clone(),
                        },
                        SwapOperation::PrismSwap {
                            offer_asset_info: prism_asset_info,
                            ask_asset_info: pluna_asset_info.clone(),
                        },
                    ],
                    minimum_receive: None,
                    to: Some(info.sender.clone()),
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
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anc0000uusd".to_string(),
                amount: Uint128::new(amount2),
                msg: to_binary(&AstroPairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: Some(astro_max_spread()),
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
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
                receiver: info.sender,
                prev_base_balance: Uint128::zero(),
                dest_asset_info: pluna_asset_info,
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_convert_and_send_with_existing_uusd_balance() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let amount1 = 100u128;
    let yluna_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        amount: Uint128::from(amount1),
    };
    let amount2 = 200u128;
    let anc_asset = Asset {
        info: AssetInfo::Cw20(Addr::unchecked("anc0000")),
        amount: Uint128::from(amount2),
    };
    configure_default_pairs(&mut deps.querier);

    // add some uusd to the contract
    let existing_uusd_balance = 1000u128;
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(existing_uusd_balance),
        },
    )]);

    // ConvertAndSend (yluna, anc)-> prism:
    // 1 - TransferFrom yluna
    // 2 - TransferFrom anc
    // 3 - prismswap swap yluna -> prism
    // 4 - astroport swap anc -> uusd
    // 5 - base hook - verify this contains the existing balance
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![yluna_asset, anc_asset],
        receiver: None,
        dest_asset_info: prism_asset_info.clone(),
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 5);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(amount1),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: "addr0000".to_string(),
                amount: Uint128::new(amount2),
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
                amount: Uint128::new(amount1),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: Some(info.sender.to_string()),
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
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anc0000uusd".to_string(),
                amount: Uint128::new(amount2),
                msg: to_binary(&AstroPairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: Some(astro_max_spread()),
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
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
                receiver: info.sender,
                prev_base_balance: Uint128::from(existing_uusd_balance),
                dest_asset_info: prism_asset_info,
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
pub fn test_distribute_failures() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let uusd_asset_info = AssetInfo::Native("uusd".to_string());
    let msg = ExecuteMsg::Distribute {
        asset_infos: vec![uusd_asset_info.clone()],
    };

    // no balance, no messages (not an error though)
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    assert_eq!(res.messages.len(), 0);

    // add uusd balance
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000u128),
        },
    )]);

    // failure - missing route
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::MissingRoute {
            asset: uusd_asset_info.clone(),
            dest_asset: AssetInfo::Cw20(Addr::unchecked("prism0000")),
        }
    );

    // failure - duplicate assets
    let msg = ExecuteMsg::Distribute {
        asset_infos: vec![uusd_asset_info.clone(), uusd_asset_info],
    };
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::DuplicateAssets {});
}

#[test]
fn test_distribute() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    configure_default_pairs(&mut deps.querier);
    configure_default_balances(&mut deps.querier);

    let asset_infos = vec![
        AssetInfo::Native("uusd".to_string()),
        AssetInfo::Cw20(Addr::unchecked("prism0000")),
        AssetInfo::Cw20(Addr::unchecked("yluna0000")),
        AssetInfo::Cw20(Addr::unchecked("pluna0000")),
    ];

    // Distribute
    // 1 - uusd -> prism, direct prismswap route
    // 2 - prism direct transfer to gov
    // 2 - yluna -> prism, direct prismswap route
    // 2 - yluna -> prism, direct prismswap route
    let gov_receiver = "gov0000".to_string();
    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 4);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000uusd".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    info: AssetInfo::Native("uusd".to_string()),
                    amount: Uint128::from(1000u128),
                },
                max_spread: None,
                belief_price: None,
                to: Some(gov_receiver.clone()),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1000u128),
            }],
        })),
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(1000u128),
                recipient: gov_receiver.clone(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "yluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000yluna0000".to_string(),
                amount: Uint128::new(4000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: Some(gov_receiver.to_string()),
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
            contract_addr: "pluna0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "pluna0000prism0000".to_string(),
                amount: Uint128::new(5000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: Some(gov_receiver),
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_distribute_with_hook() {
    // this situation likely not necessary, but testing anyway
    // hooks are only needed for external prism assets (e.g. anc)
    // and we typically won't have any balance in those anyway
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    configure_default_pairs(&mut deps.querier);

    deps.querier.with_token_balances(&[
        (
            &"xprism0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(2000u128))],
        ),
        (
            &"anc0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(6000u128))],
        ),
    ]);

    let asset_infos = vec![
        AssetInfo::Native("uusd".to_string()),
        AssetInfo::Cw20(Addr::unchecked("xprism0000")),
        AssetInfo::Cw20(Addr::unchecked("anc0000")),
        AssetInfo::Cw20(Addr::unchecked("pluna0000")),
    ];

    // Distribute
    // 1 - xprism -> prism, direct prismswap route
    // 2 - anc -> uusd (astroport)
    // 2 - base swap hook
    let gov_receiver = "gov0000".to_string();
    let msg = ExecuteMsg::Distribute { asset_infos };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 3);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "xprism0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "prism0000xprism0000".to_string(),
                amount: Uint128::new(2000u128),
                msg: to_binary(&PairCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: Some(gov_receiver.to_string()),
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
            contract_addr: "anc0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anc0000uusd".to_string(),
                amount: Uint128::new(6000u128),
                msg: to_binary(&AstroPairCw20HookMsg::Swap {
                    belief_price: None,
                    max_spread: Some(astro_max_spread()),
                    to: Some(MOCK_CONTRACT_ADDR.to_string()),
                })
                .unwrap(),
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
                receiver: Addr::unchecked(gov_receiver),
                prev_base_balance: Uint128::zero(),
                dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
            })
            .unwrap(),
            funds: vec![],
        }))
    );
}

#[test]
fn test_base_swap_hook_to_prism() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::BaseSwapHook {
        receiver: Addr::unchecked("gov0000"),
        prev_base_balance: Uint128::zero(),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::from(StdError::generic_err("unauthorized"))
    );

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
            asset: AssetInfo::Native("uusd".to_string()),
            dest_asset: AssetInfo::Cw20(Addr::unchecked("prism0000")),
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
}

#[test]
fn test_base_swap_hook_to_yluna() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::BaseSwapHook {
        receiver: Addr::unchecked("gov0000"),
        prev_base_balance: Uint128::zero(),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("yluna0000")),
    };

    // add some uusd balance to simulate the initial swap results
    deps.querier.with_native_balances(&[(
        MOCK_CONTRACT_ADDR.to_string(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(11134u128),
        },
    )]);

    configure_default_pairs(&mut deps.querier);

    let uusd_asset_info = AssetInfo::Native("uusd".to_string());
    let prism_asset_info = AssetInfo::Cw20(Addr::unchecked("prism0000"));
    let yluna_asset_info = AssetInfo::Cw20(Addr::unchecked("yluna0000"));

    // run the base swap hook, this results in a router swap
    // from uusd -> prism -> yluna:
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "base_swap_hook")]);
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prismrouter0000".to_string(),
            msg: to_binary(&RouterExecuteMsg::ExecuteSwapOperations {
                operations: vec![
                    SwapOperation::PrismSwap {
                        offer_asset_info: uusd_asset_info,
                        ask_asset_info: prism_asset_info.clone(),
                    },
                    SwapOperation::PrismSwap {
                        offer_asset_info: prism_asset_info,
                        ask_asset_info: yluna_asset_info,
                    },
                ],
                minimum_receive: None,
                to: Some(Addr::unchecked("gov0000")),
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(11134u128),
            }],
        })),
    );
}

#[test]
fn test_base_swap_hook_with_prev_base_balance() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let msg = ExecuteMsg::BaseSwapHook {
        receiver: Addr::unchecked("gov0000"),
        prev_base_balance: Uint128::from(5000u128),
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
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
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
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
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
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
        dest_asset_info: AssetInfo::Cw20(Addr::unchecked("prism0000")),
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
