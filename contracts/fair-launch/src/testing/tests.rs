use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockQuerier};
use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, BankMsg,
    MemoryStorage, OwnedDeps, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::contract::{deposit, query_deposit_info, instantiate, post_initialize, withdraw};
use prism_protocol::fair_launch::{
    DepositResponse, InstantiateMsg, LaunchConfig,
};
use terra_cosmwasm::{TerraQueryWrapper};
use crate::testing::mock_querier::{mock_dependencies};

pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, MockQuerier<TerraQueryWrapper>>) {
    //deps.querier.with_custom_handler(|query| SystemResult::Ok(custom_query_execute(&query)));
    let msg = InstantiateMsg {
        owner: "owner0001".to_string(),
        token: "prism0001".to_string(),
    };

    let info = mock_info("owner0001", &[]);
    let env = mock_env();
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
}

pub fn post_init(deps: &mut OwnedDeps<MemoryStorage, MockApi, MockQuerier<TerraQueryWrapper>>) {
    let info = mock_info("owner0001", &[]);
    let env = mock_env();
    let launch_config = LaunchConfig {
        amount: Uint128::from(1_000_000u64),
        phase1_start: env.block.time.seconds(),
        phase2_start: env.block.time.seconds() + 100,
        phase2_end: env.block.time.seconds() + 200,
    };
    post_initialize(
        deps.as_mut(), 
        env.clone(), 
        info.clone(), 
        launch_config.clone()
    ).unwrap();
}

#[test]
fn proper_post_initialize() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let mut info = mock_info("", &[]);
    let env = mock_env();
    let mut launch_config = LaunchConfig {
        amount: Uint128::from(1_000_000u64),
        phase1_start: env.block.time.seconds(),
        phase2_start: env.block.time.seconds() + 100,
        phase2_end: env.block.time.seconds() + 200,
    };

    // unauthorized
    let err = post_initialize(deps.as_mut(), env.clone(), info.clone(), launch_config.clone());
    assert_eq!(err.unwrap_err(), ContractError::Unauthorized{});

    // invalid launch config
    info.sender = Addr::unchecked("owner0001");
    launch_config.phase1_start = env.block.time.seconds() - 100;
    let err = post_initialize(deps.as_mut(), env.clone(), info.clone(), launch_config.clone());
    assert_eq!(err.unwrap_err(), ContractError::InvalidLaunchConfig{});

    // success
    launch_config.phase1_start = env.block.time.seconds();
    let res = post_initialize(deps.as_mut(), env.clone(), info.clone(), launch_config.clone());
    assert!(res.is_ok());
    let msgs = res.unwrap().messages;
    assert_eq!(msgs.len(), 1);
    let msg = &msgs[0].msg;
    match msg {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds: _,
        }) => {
          assert_eq!(contract_addr, "prism0001");
          assert_eq!(
              msg,
              &to_binary(&Cw20ExecuteMsg::TransferFrom {
                  owner: "owner0001".to_string(),
                  recipient: env.contract.address.to_string(),
                  amount: Uint128::from(1_000_000u64)
              })
              .unwrap()
          )
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };
}

#[test]
fn proper_deposit() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    post_init(&mut deps);

    let mut info = mock_info("addr0001", &[]);
    let mut env = mock_env();

    // error, no coins sent with deposit
    let err = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(err.unwrap_err(), ContractError::InvalidDeposit{ 
        reason: "requires 1 coin deposited".to_string()
    });
    
    // error, zero amount
    info.funds = vec![Coin::new(0, "uusd")];
    let err = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(err.unwrap_err(), ContractError::InvalidDeposit{
        reason: "requires uusd and positive amount".to_string()
    });

    // error, wrong currency
    info.funds = vec![Coin::new(0, "ukrw")];
    let err = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(err.unwrap_err(), ContractError::InvalidDeposit{
        reason: "requires uusd and positive amount".to_string()
    });

    // successful deposit
    info.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    // query deposit responses for addr0001
    let deposit_info = query_deposit_info(deps.as_ref(), "addr0001".to_string());
    assert_eq!(deposit_info.unwrap(), DepositResponse {
        address_deposit: Uint128::from(1_000u128),
        total_deposit: Uint128::from(1_000u128),
    });

    // query deposit responses for addr0002
    let deposit_info = query_deposit_info(deps.as_ref(), "addr0002".to_string());
    assert_eq!(deposit_info.unwrap(), DepositResponse {
        address_deposit: Uint128::zero(),
        total_deposit: Uint128::from(1_000u128),
    });

    // failed deposit, after phase 1
    env.block.time = env.block.time.plus_seconds(250u64);
    let err = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(err.unwrap_err(), ContractError::InvalidDeposit{
        reason: "deposit period is over".to_string()
    });
}


#[test]
fn proper_withdraw() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    post_init(&mut deps);

    let mut info = mock_info("addr0001", &[]);
    let env = mock_env();

    // successful deposit
    info.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    // successful withdraw 100, we get 95 due to 5% taxes inside mock_querier
    let res = withdraw(deps.as_mut(), env.clone(), info.clone(), Uint128::from(100u128)).unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Bank(BankMsg::Send {
            to_address,
            amount,
        }) => {
          assert_eq!(to_address, info.sender.as_str());
          assert_eq!(amount[0], Coin{ denom: "uusd".to_string(), amount: Uint128::from(95u128) });
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };


    // query deposit responses for addr0001
    let deposit_info = query_deposit_info(deps.as_ref(), "addr0001".to_string());
    assert_eq!(deposit_info.unwrap(), DepositResponse {
        address_deposit: Uint128::from(900u128),
        total_deposit: Uint128::from(1_000u128),
    });
}
