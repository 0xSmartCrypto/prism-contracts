use super::mock_querier::mock_dependencies;
use crate::contract::{execute, instantiate, query_config};
use astroport::asset::{Asset, AssetInfo};
use astroport::pair::{Cw20HookMsg as PairCw20HookMsg, ExecuteMsg as PairExecuteMsg};
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, to_binary, Addr, Coin, CosmosMsg, Decimal, StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        prism_base_pair: "prismuusdpair0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = query_config(deps.as_ref()).unwrap();
    assert_eq!("astrofactory0000", config.astroport_factory.as_str());
    assert_eq!("gov0000", config.distribution_contract.as_str());
    assert_eq!("prismuusdpair0000", config.prism_base_pair.as_str());
    assert_eq!("prism0000", config.prism_token.as_str());
    assert_eq!("uusd", config.base_denom.as_str());
}

#[test]
fn test_convert_and_send() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::new(100u128),
    }]);

    deps.querier.with_pairs(&[
        (
            &"prism0000yluna0000".to_string(),
            &"yLunaPair0000".to_string(),
        ),
        (&"uusdanc0000".to_string(), &"ancPair0000".to_string()),
    ]);

    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        prism_base_pair: "prismuusdpair0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::ConvertAndSend {
        assets: vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("yluna0000"),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("anc0000"),
                },
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
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "yLunaPair0000".to_string(),
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
                    contract: "ancPair0000".to_string(),
                    amount: Uint128::new(200u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
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
fn test_distribute() {
    let mut deps = mock_dependencies(&[]);
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
        (
            &"prism0000yluna0000".to_string(),
            &"yLunaPair0000".to_string(),
        ),
        (&"uusdanc0000".to_string(), &"ancPair0000".to_string()),
    ]);

    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        prism_base_pair: "prismuusdpair0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::Distribute {
        asset_tokens: vec!["yluna0000".to_string(), "anc0000".to_string()],
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "yluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "yLunaPair0000".to_string(),
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
                    contract: "ancPair0000".to_string(),
                    amount: Uint128::new(200u128),
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
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
    )
}

#[test]
fn test_base_swap_hook() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(11134u128),
    }]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::new(1000000u128))],
    );

    let msg = InstantiateMsg {
        astroport_factory: "astrofactory0000".to_string(),
        distribution_contract: "gov0000".to_string(),
        prism_token: "prism0000".to_string(),
        prism_base_pair: "prismuusdpair0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::BaseSwapHook { receiver: None };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized"));

    // successfull attempt
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "base_swap_hook")]);
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "prismuusdpair0000".to_string(),
            msg: to_binary(&PairExecuteMsg::Swap {
                offer_asset: Asset {
                    amount: Uint128::from(11023u128), // 11134 - 1% tax
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string()
                    },
                },
                max_spread: None,
                belief_price: None,
                to: Some("gov0000".to_string()), // by default sends to gov
            })
            .unwrap(),
            funds: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(11023u128),
            }],
        }))]
    )
}
