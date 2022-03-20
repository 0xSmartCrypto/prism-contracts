use cosmwasm_std::{
    coin, from_binary, to_binary, Api, CosmosMsg, DepsMut, Env, MessageInfo, OwnedDeps, Querier,
    Response, Storage, SubMsg, Uint128, WasmMsg,
};

use cosmwasm_std::testing::{mock_env, mock_info};

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use prism_protocol::basset_vault::{
    BondedAmountResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg,
    StateResponse,
};
use prism_protocol::reward_distribution::ExecuteMsg as RewardDistributionExecuteMsg;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use beth::reward::ExecuteMsg as BassetRewardExecuteMsg;
use prism_common::testing::mock_querier::{mock_dependencies as dependencies, MOCK_CONTRACT_ADDR};

const OWNER: &str = "owner";
const OWNER2: &str = "owner2";
const BASSET_CONTRACT: &str = "beth";
const BASSET_REWARD_CONTRACT: &str = "beth_reward";
const BASSET_REWARD_DENOM: &str = "uusd";
const CASSET_CONTRACT: &str = "cbeth";
const PASSET_CONTRACT: &str = "pbeth";
const YASSET_CONTRACT: &str = "ybeth";
const REWARD_DISTRIBUTION_CONTRACT: &str = "reward_distribution";
const BOB_ADDR: &str = "bob";

pub fn init<S: Storage, A: Api, Q: Querier>(deps: &mut OwnedDeps<S, A, Q>) {
    let msg = InstantiateMsg {
        asset_contract: BASSET_CONTRACT.to_string(),
        asset_reward_contract: BASSET_REWARD_CONTRACT.to_string(),
        asset_reward_denom: BASSET_REWARD_DENOM.to_string(),
    };

    let owner_info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    let register_msg = ExecuteMsg::UpdateConfig {
        owner: None,
        casset_contract: Some(CASSET_CONTRACT.to_string()),
        yasset_contract: Some(YASSET_CONTRACT.to_string()),
        passet_contract: Some(PASSET_CONTRACT.to_string()),
        reward_distribution_contract: Some(REWARD_DISTRIBUTION_CONTRACT.to_string()),
    };

    execute(deps.as_mut(), mock_env(), owner_info, register_msg).unwrap();
}

pub fn do_bond(
    deps: DepsMut,
    addr: String,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Response {
    let bond = Cw20HookMsg::Bond {};
    let receive = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: addr,
        amount,
        msg: to_binary(&bond).unwrap(),
    });
    execute(deps, env, info, receive).unwrap()
}

/// Covers if all the fields of InitMsg are stored in
/// parameters' storage, the config storage stores the creator,
/// the current batch storage and state are initialized.
#[test]
fn test_initialization() {
    let mut deps = dependencies(&[]);

    // successful call
    let msg = InstantiateMsg {
        asset_contract: BASSET_CONTRACT.to_string(),
        asset_reward_contract: BASSET_REWARD_CONTRACT.to_string(),
        asset_reward_denom: BASSET_REWARD_DENOM.to_string(),
    };

    // not payable error
    let owner_info = mock_info(OWNER, &[coin(1000000, "uluna")]);
    let res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::NonPayable {});

    // successful initialization
    let owner_info = mock_info(OWNER, &[]);
    instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    // state storage must be initialized
    let state = QueryMsg::State {};
    let query_state: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), state).unwrap()).unwrap();
    let expected_result = StateResponse {
        total_bond_amount: Uint128::zero(),
        last_index_modification: mock_env().block.time.seconds(),
    };
    assert_eq!(query_state, expected_result);

    // config storage must be initialized
    let conf = QueryMsg::Config {};
    let query_conf: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), conf).unwrap()).unwrap();
    let expected_conf = ConfigResponse {
        owner: OWNER.to_string(),
        asset_contract: BASSET_CONTRACT.to_string(),
        asset_reward_contract: BASSET_REWARD_CONTRACT.to_string(),
        asset_reward_denom: BASSET_REWARD_DENOM.to_string(),
        casset_contract: None,
        yasset_contract: None,
        passet_contract: None,
        reward_distribution_contract: None,
    };
    assert_eq!(expected_conf, query_conf);

    let update_config_msg = ExecuteMsg::UpdateConfig {
        owner: Some(OWNER2.to_string()),
        casset_contract: Some(CASSET_CONTRACT.to_string()),
        yasset_contract: Some(YASSET_CONTRACT.to_string()),
        passet_contract: Some(PASSET_CONTRACT.to_string()),
        reward_distribution_contract: Some(REWARD_DISTRIBUTION_CONTRACT.to_string()),
    };

    // unauthorized UpdateConfig
    let unauthorized_info = mock_info("unauthorized", &[]);
    let res = execute(
        deps.as_mut(),
        mock_env(),
        unauthorized_info,
        update_config_msg.clone(),
    )
    .unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // successful UpdateConfig
    execute(
        deps.as_mut(),
        mock_env(),
        owner_info.clone(),
        update_config_msg.clone(),
    )
    .unwrap();

    // query config, verify all fields
    let conf = QueryMsg::Config {};
    let query_conf: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), conf).unwrap()).unwrap();
    let expected_conf = ConfigResponse {
        owner: OWNER2.to_string(),
        asset_contract: BASSET_CONTRACT.to_string(),
        asset_reward_contract: BASSET_REWARD_CONTRACT.to_string(),
        asset_reward_denom: BASSET_REWARD_DENOM.to_string(),
        casset_contract: Some(CASSET_CONTRACT.to_string()),
        yasset_contract: Some(YASSET_CONTRACT.to_string()),
        passet_contract: Some(PASSET_CONTRACT.to_string()),
        reward_distribution_contract: Some(REWARD_DISTRIBUTION_CONTRACT.to_string()),
    };
    assert_eq!(expected_conf, query_conf);

    // unauthorized UpdateConfig, contract now owned by OWNER2
    let res = execute(
        deps.as_mut(),
        mock_env(),
        owner_info,
        update_config_msg.clone(),
    )
    .unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // duplicate update config
    let owner_info = mock_info(OWNER2, &[]);
    let res = execute(
        deps.as_mut(),
        mock_env(),
        owner_info,
        update_config_msg.clone(),
    )
    .unwrap_err();
    assert_eq!(res, ContractError::DuplicateUpdateConfig {});
}

#[test]
fn test_bond() {
    let mut deps = dependencies(&[]);

    init(&mut deps);
    let bond_amount = Uint128::from(1000u128);
    let bond_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB_ADDR.to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    // failed bond, cw20 send from wrong contract (should be basset contract)
    let info = mock_info(CASSET_CONTRACT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, bond_msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // successful bond
    let info = mock_info(BASSET_CONTRACT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), bond_msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: CASSET_CONTRACT.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: BOB_ADDR.to_string(),
                amount: bond_amount,
            })
            .unwrap(),
            funds: vec![]
        })),]
    );

    // query and verify state
    let query_msg = QueryMsg::State {};
    let res: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
    let expected = StateResponse {
        total_bond_amount: bond_amount,
        last_index_modification: mock_env().block.time.seconds(),
    };
    assert_eq!(res, expected);

    // query and verify state
    let query_msg = QueryMsg::BondedAmount {};
    let res: BondedAmountResponse =
        from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
    let expected = BondedAmountResponse {
        total_bond_amount: bond_amount,
    };
    assert_eq!(res, expected);
}

#[test]
fn test_bond_split() {
    let mut deps = dependencies(&[]);

    init(&mut deps);
    let bond_amount = Uint128::from(1000u128);
    let bond_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB_ADDR.to_string(),
        amount: bond_amount,
        msg: to_binary(&Cw20HookMsg::BondSplit {}).unwrap(),
    });

    // failed bond split, cw20 send from wrong contract (should be basset contract)
    let info = mock_info(CASSET_CONTRACT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, bond_msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // successful bond split
    let info = mock_info(BASSET_CONTRACT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), bond_msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: CASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: PASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
        ]
    );

    // query and verify state
    let query_msg = QueryMsg::State {};
    let res: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
    let expected = StateResponse {
        total_bond_amount: bond_amount,
        last_index_modification: mock_env().block.time.seconds(),
    };
    assert_eq!(res, expected);
}

#[test]
#[should_panic(expected = "unbond amount can not be more than stored total bonded amount")]
fn test_unbond() {
    let mut deps = dependencies(&[]);

    init(&mut deps);

    // successful bond
    let bond_amount = Uint128::from(1000u128);
    let info = mock_info(BASSET_CONTRACT, &[]);
    do_bond(
        deps.as_mut(),
        BOB_ADDR.to_string(),
        mock_env(),
        info.clone(),
        bond_amount,
    );

    let unbond_amount = Uint128::from(500u128);
    let unbond_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB_ADDR.to_string(),
        amount: unbond_amount,
        msg: to_binary(&Cw20HookMsg::Unbond {}).unwrap(),
    });

    // failed unbond, cw20 send from wrong contract (should be casset contract)
    let res = execute(deps.as_mut(), mock_env(), info, unbond_msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // successful unbond of half (500)
    let info = mock_info(CASSET_CONTRACT, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, unbond_msg.clone()).unwrap();
    assert_eq!(res.messages.len(), 2);
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: CASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: unbond_amount
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: BASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: BOB_ADDR.to_string(),
                    amount: unbond_amount
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    // query and verify state
    let query_msg = QueryMsg::State {};
    let res: StateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
    let expected = StateResponse {
        total_bond_amount: bond_amount - unbond_amount,
        last_index_modification: mock_env().block.time.seconds(),
    };
    assert_eq!(res, expected);

    // failed unbond of 600, we only have 500 bonded, this panics, should never happen...
    let unbond_amount = Uint128::from(600u128);
    let unbond_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB_ADDR.to_string(),
        amount: unbond_amount,
        msg: to_binary(&Cw20HookMsg::Unbond {}).unwrap(),
    });
    let info = mock_info(CASSET_CONTRACT, &[]);
    execute(deps.as_mut(), mock_env(), info, unbond_msg.clone()).unwrap();
}

#[test]
fn test_bond_split_merge() {
    let mut deps = dependencies(&[]);

    init(&mut deps);

    // successful bond
    let bond_amount = Uint128::from(1000u128);
    let info = mock_info(BASSET_CONTRACT, &[]);
    do_bond(
        deps.as_mut(),
        BOB_ADDR.to_string(),
        mock_env(),
        info.clone(),
        bond_amount,
    );

    // successful split
    let info = mock_info(BOB_ADDR, &[]);
    let split_msg = ExecuteMsg::Split {
        amount: bond_amount,
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), split_msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: CASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: BOB_ADDR.to_string(),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: PASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
        ]
    );

    // successful merge
    let split_msg = ExecuteMsg::Merge {
        amount: bond_amount,
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), split_msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: CASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: YASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                    owner: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: PASSET_CONTRACT.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                    owner: BOB_ADDR.to_string(),
                    amount: bond_amount,
                })
                .unwrap(),
                funds: vec![]
            })),
        ]
    );
}

#[test]
pub fn test_update_global_index() {
    let mut deps = dependencies(&[]);

    init(&mut deps);

    // fails if there is no delegation
    let msg = ExecuteMsg::UpdateGlobalIndex {};
    let info = mock_info(BOB_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: BASSET_REWARD_CONTRACT.to_string(),
                msg: to_binary(&BassetRewardExecuteMsg::ClaimRewards {
                    recipient: Some(REWARD_DISTRIBUTION_CONTRACT.to_string()),
                })
                .unwrap(),
                funds: vec![]
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: REWARD_DISTRIBUTION_CONTRACT.to_string(),
                msg: to_binary(&RewardDistributionExecuteMsg::DistributeRewards {})
                .unwrap(),
                funds: vec![]
            })),
        ]
    );

    let env = mock_env();
    let state_response: StateResponse =
        from_binary(&query(deps.as_ref(), env.clone(), QueryMsg::State {}).unwrap()).unwrap();
    assert_eq!(
        state_response,
        StateResponse {
            total_bond_amount: Uint128::zero(),
            last_index_modification: env.block.time.seconds(),
        }
    );
}
