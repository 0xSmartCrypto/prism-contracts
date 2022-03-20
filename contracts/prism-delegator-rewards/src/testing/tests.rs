use cosmwasm_std::{
    attr, from_binary, to_binary, Api, Coin, CosmosMsg,  OwnedDeps, Querier,
    Storage, SubMsg, Uint128, WasmMsg,
};

use cosmwasm_std::testing::{mock_env, mock_info};

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use prism_protocol::delegator_rewards::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg
};
use prism_protocol::vault::ExecuteMsg as VaultExecuteMsg;
use prism_protocol::reward_distribution::ExecuteMsg as RewardDistributionExecuteMsg;

use prism_common::testing::mock_querier::{
    mock_dependencies, MOCK_CONTRACT_ADDR, VAULT
};
use terra_cosmwasm::create_swap_msg;
use cw20::Cw20ExecuteMsg;

const OWNER: &str = "owner";
const YLUNA_TOKEN: &str = "yluna";
const PLUNA_TOKEN: &str = "pluna";
const REWARD_DISTRIBUTION: &str = "reward_distribution";
const DELEGATOR_REWARD_DENOM: &str = "uluna";

pub fn init<S: Storage, A: Api, Q: Querier>(deps: &mut OwnedDeps<S, A, Q>) {
    let msg = InstantiateMsg {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        yluna_token: YLUNA_TOKEN.to_string(),
        pluna_token: PLUNA_TOKEN.to_string(),
        reward_distribution: REWARD_DISTRIBUTION.to_string(),
    };

    let owner_info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();
}

#[test]
fn test_initialization() {
    let mut deps = mock_dependencies(&[]);

    // valid init
    init(&mut deps);

    // verify config storage
    let state = QueryMsg::Config {};
    let config_response: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = ConfigResponse {
        owner: OWNER.to_string(),
        vault: VAULT.to_string(),
        yluna_token: YLUNA_TOKEN.to_string(),
        pluna_token: PLUNA_TOKEN.to_string(),
        reward_distribution: REWARD_DISTRIBUTION.to_string(),
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
                msg: to_binary(&ExecuteMsg::LunaToPylunaHook {})
                .unwrap(),
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
                msg: to_binary(&ExecuteMsg::DistributeMintedPylunaHook {})
                .unwrap(),
                funds: vec![],
            })),
        ]
    )
}

#[test]
fn test_distribute_minted_pyluna_hook() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let msg = ExecuteMsg::DistributeMintedPylunaHook {};

    deps.querier.with_token_balances(&[
        (
            &PLUNA_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(2_000_000u128))],
        ),
        (
            &YLUNA_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1_000_000u128))],
        ),
    ]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![attr("action", "distribute_minted_pyluna_hook")]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: PLUNA_TOKEN.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: REWARD_DISTRIBUTION.to_string(),
                        amount: Uint128::from(2_000_000u128),
                    }).unwrap(),
                    funds: vec![],
                }),
            ),
            SubMsg::new(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: YLUNA_TOKEN.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: REWARD_DISTRIBUTION.to_string(),
                        amount: Uint128::from(1_000_000u128),
                    }).unwrap(),
                    funds: vec![],
                }),
            ),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_DISTRIBUTION.to_string(),
                msg: to_binary(&RewardDistributionExecuteMsg::DistributeRewards {})
                .unwrap(),
                funds: vec![],
            })),

        ]
    )
}