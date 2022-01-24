use cosmwasm_std::testing::{mock_env, mock_info, MockApi};
use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, MemoryStorage, OwnedDeps, SubMsg,
    Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::contract::{
    admin_withdraw, deposit, instantiate, post_initialize, query_deposit_info, release_tokens,
    withdraw, withdraw_tokens, SECONDS_PER_HOUR,
};
use crate::error::ContractError;
use prism_common::testing::mock_querier::{mock_dependencies, WasmMockQuerier, MOCK_CONTRACT_ADDR};
use prism_protocol::fair_launch::{DepositResponse, InstantiateMsg, LaunchConfig};

pub fn init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let msg = InstantiateMsg {
        owner: "owner0001".to_string(),
        receiver: "receiver0000".to_string(),
        token: "prism0001".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("owner0001", &[]);
    let env = mock_env();
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
}

pub fn post_init(deps: &mut OwnedDeps<MemoryStorage, MockApi, WasmMockQuerier>) {
    let info = mock_info("owner0001", &[]);
    let env = mock_env();
    let launch_config = LaunchConfig {
        amount: Uint128::from(1_000_000u64),
        phase1_start: env.block.time.seconds(),
        phase2_start: env.block.time.seconds() + 100,
        phase2_end: env.block.time.seconds() + 100 + SECONDS_PER_HOUR,
    };
    post_initialize(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        launch_config.clone(),
    )
    .unwrap();
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
        phase2_end: env.block.time.seconds() + 100 + SECONDS_PER_HOUR,
    };

    // unauthorized
    let err = post_initialize(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        launch_config.clone(),
    );
    assert_eq!(err.unwrap_err(), ContractError::Unauthorized {});

    // invalid launch config
    info.sender = Addr::unchecked("owner0001");
    launch_config.phase1_start = env.block.time.seconds() - 100;
    let err = post_initialize(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        launch_config.clone(),
    );
    assert_eq!(err.unwrap_err(), ContractError::InvalidLaunchConfig {});

    // success
    launch_config.phase1_start = env.block.time.seconds();
    let res = post_initialize(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        launch_config.clone(),
    );
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
    assert_eq!(
        err.unwrap_err(),
        ContractError::InvalidDeposit {
            reason: "requires 1 coin deposited".to_string()
        }
    );

    // error, zero amount
    info.funds = vec![Coin::new(0, "uusd")];
    let err = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(
        err.unwrap_err(),
        ContractError::InvalidDeposit {
            reason: "requires uusd and positive amount".to_string()
        }
    );

    // error, wrong currency
    info.funds = vec![Coin::new(0, "ukrw")];
    let err = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(
        err.unwrap_err(),
        ContractError::InvalidDeposit {
            reason: "requires uusd and positive amount".to_string()
        }
    );

    // successful deposit
    info.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    // query deposit responses for addr0001
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "addr0001".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(1_000u128),
            total_deposit: Uint128::from(1_000u128),
            withdrawable_amount: Uint128::from(1_000u128),
            tokens_to_claim: Uint128::from(1_000_000u64),
            can_claim: false,
        }
    );

    // query deposit responses for addr0002
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "addr0002".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::zero(),
            total_deposit: Uint128::from(1_000u128),
            withdrawable_amount: Uint128::zero(),
            tokens_to_claim: Uint128::zero(),
            can_claim: false,
        }
    );

    // failed deposit, after phase 1
    env.block.time = env.block.time.plus_seconds(150u64);
    let err = deposit(deps.as_mut(), env, info);
    assert_eq!(
        err.unwrap_err(),
        ContractError::InvalidDeposit {
            reason: "deposit period is over".to_string()
        }
    );
}

#[test]
fn proper_withdraw() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    post_init(&mut deps);

    let caps = [(&"uusd".to_string(), &Uint128::from(10u128))];
    deps.querier.with_tax(Decimal::percent(5u64), &caps);

    let mut info = mock_info("addr0001", &[]);
    let mut env = mock_env();

    // successful deposit
    info.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    // try to withdraw 0, expect error
    let err = withdraw(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        Some(Uint128::zero()),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdraw {
            reason: "withdraw amount must be bigger than 0".to_string()
        }
    );

    // successful withdraw 100, we get 95 due to 5% taxes configured inside mock_querier
    let res = withdraw(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        Some(Uint128::from(100u128)),
    )
    .unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, info.sender.as_str());
            assert_eq!(
                amount[0],
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(95u128)
                }
            );
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };

    // query deposit responses for addr0001
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "addr0001".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(900u128),
            total_deposit: Uint128::from(900u128),
            withdrawable_amount: Uint128::from(900u128),
            tokens_to_claim: Uint128::from(1_000_000u64),
            can_claim: false,
        }
    );

    // successful withdraw remaining, we have 900 left, we get 890 (tax cap = 10)
    let res = withdraw(deps.as_mut(), env.clone(), info.clone(), None).unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, info.sender.as_str());
            assert_eq!(
                amount[0],
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(890u128)
                }
            );
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };

    // query deposit responses for addr0001
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "addr0001".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::zero(),
            total_deposit: Uint128::zero(),
            withdrawable_amount: Uint128::zero(),
            tokens_to_claim: Uint128::zero(),
            can_claim: false,
        }
    );

    let err = withdraw(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        Some(Uint128::from(1_000u128)),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdraw {
            reason: "no funds available to withdraw".to_string()
        }
    );

    // successful deposit again
    info.funds = vec![Coin::new(200_000_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    // execute another withdraw
    let res = withdraw(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        Some(Uint128::from(101_000_000u128)),
    )
    .unwrap();
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, info.sender.as_str());
            assert_eq!(
                amount[0],
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(100999990u128) // 101_000_000 - tax_cap
                }
            );
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };

    // fast forward to after phase 2, withdraws not allowed
    env.block.time = env.block.time.plus_seconds(100 + SECONDS_PER_HOUR);
    let err = withdraw(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        Some(Uint128::from(100u128)),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdraw {
            reason: "withdraw period is over".to_string()
        }
    );

    // withdrawable amount should be zero
    let deposit_info = query_deposit_info(deps.as_ref(), env, "addr0001".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(99_000_000u128),
            total_deposit: Uint128::from(99_000_000u128),
            withdrawable_amount: Uint128::zero(),
            tokens_to_claim: Uint128::from(1_000_000u64),
            can_claim: false, // phase 2 is over, but tokens not released, so cant claim yet
        }
    );
}

#[test]
fn proper_withdraw_phase3() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);

    let env = mock_env();
    let info = mock_info("owner0001", &[]);
    let launch_config = LaunchConfig {
        amount: Uint128::from(1_000_000u64),
        phase1_start: env.block.time.seconds(),
        phase2_start: env.block.time.seconds() + 100,
        phase2_end: env.block.time.seconds() + 100 + 24 * SECONDS_PER_HOUR, // 24 hour phase 2
    };
    post_initialize(deps.as_mut(), mock_env(), info, launch_config).unwrap();

    let caps = [(&"uusd".to_string(), &Uint128::from(10u128))];
    deps.querier.with_tax(Decimal::percent(5u64), &caps);

    let mut alice_info = mock_info("alice0000", &[]);
    let mut bob_info = mock_info("bob0000", &[]);
    let mut cindy_info = mock_info("cindy0000", &[]);
    let mut env = mock_env();

    // successful deposit with 3 accounts
    alice_info.funds = vec![Coin::new(100_000_000, "uusd")];
    deposit(deps.as_mut(), env.clone(), alice_info.clone()).unwrap();
    bob_info.funds = vec![Coin::new(100_000_000, "uusd")];
    deposit(deps.as_mut(), env.clone(), bob_info.clone()).unwrap();
    cindy_info.funds = vec![Coin::new(100_000_000, "uusd")];
    deposit(deps.as_mut(), env.clone(), cindy_info.clone()).unwrap();

    // fast forward to phase 2
    env.block.time = env.block.time.plus_seconds(101);

    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "alice0000".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(100_000_000u128),
            total_deposit: Uint128::from(300_000_000u128),
            withdrawable_amount: Uint128::from(100_000_000u128),
            tokens_to_claim: Uint128::from(333333u128),
            can_claim: false,
        }
    );

    // try to withdraw more than withdrawable
    let err = withdraw(
        deps.as_mut(),
        env.clone(),
        alice_info.clone(),
        Some(Uint128::from(100_000_001u128)),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdraw {
            reason: "can not withdraw more than current withrawable amount (100000000)".to_string()
        }
    );

    // valid withdraw
    let res = withdraw(
        deps.as_mut(),
        env.clone(),
        alice_info.clone(),
        Some(Uint128::from(1_000_000u128)),
    )
    .unwrap();
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, alice_info.sender.as_str());
            assert_eq!(
                amount[0],
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(999990u128) // 999_990 - tax_cap
                }
            );
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };

    // try to withdraw again, expect error
    let err = withdraw(
        deps.as_mut(),
        env.clone(),
        alice_info.clone(),
        Some(Uint128::from(1_000u128)),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdraw {
            reason: "a withdraw was already executed on phase 2".to_string()
        }
    );

    // fast forward 6 hours
    env.block.time = env.block.time.plus_seconds(SECONDS_PER_HOUR * 6);
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "bob0000".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(100_000_000u128),
            total_deposit: Uint128::from(299_000_000u128),
            withdrawable_amount: Uint128::from(75_000_000u128), // 100M * 18/24 70833333
            tokens_to_claim: Uint128::from(334448u128),         // 100000000 / 299000000 * 1000000
            can_claim: false,
        }
    );
    // valid withdraw all remaining
    let res = withdraw(deps.as_mut(), env.clone(), bob_info.clone(), None).unwrap();
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, bob_info.sender.as_str());
            assert_eq!(
                amount[0],
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(74999990u128) // 75_000_000 - tax_cap
                }
            );
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };

    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "bob0000".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(25000000u128), // 100000000 - 75000000
            total_deposit: Uint128::from(224000000u128),
            withdrawable_amount: Uint128::zero(), // can not withraw more, only one time
            tokens_to_claim: Uint128::from(111607u128), // 25000000 / 224000000 * 1000000
            can_claim: false,
        }
    );

    // last slot of phase 2
    env.block.time = env.block.time.plus_seconds(SECONDS_PER_HOUR * 17 + 3598);
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "cindy0000".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(100000000u128),
            total_deposit: Uint128::from(224000000u128),
            withdrawable_amount: Uint128::from(4166666u128), // 100000000 * 1 / 24
            tokens_to_claim: Uint128::from(446428u128), // 100000000 / 224000000 * 1000000
            can_claim: false,
        }
    );

    // after phase 2
    env.block.time = env.block.time.plus_seconds(1);
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "cindy0000".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(100000000u128),
            total_deposit: Uint128::from(224000000u128),
            withdrawable_amount: Uint128::zero(), // 100000000 * 0 / 24
            tokens_to_claim: Uint128::from(446428u128), // 100000000 / 224000000 * 1000000
            can_claim: false,                     // tokens not released, cant claim tokens yet
        }
    );
}

#[test]
fn proper_withdraw_tokens1() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    post_init(&mut deps);

    let mut info = mock_info("addr0001", &[]);
    let mut env = mock_env();

    // successful deposit
    info.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    let err = withdraw_tokens(deps.as_mut(), env.clone(), info.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdrawTokens {
            reason: "cannot withdraw tokens yet".to_string()
        }
    );

    // fast forward past phase 2
    env.block.time = env.block.time.plus_seconds(100 + SECONDS_PER_HOUR);

    // tokens have not yet been released, so should still return error
    let err = withdraw_tokens(deps.as_mut(), env.clone(), info.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdrawTokens {
            reason: "cannot withdraw tokens yet".to_string()
        }
    );

    // admin executes release tokens --  unauthorized attempt
    let err = release_tokens(deps.as_mut(), env.clone(), info.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // valid attempt
    let owner_info = mock_info("owner0001", &[]);
    release_tokens(deps.as_mut(), env.clone(), owner_info).unwrap();

    // now users can withdraw tokens
    let res = withdraw_tokens(deps.as_mut(), env.clone(), info.clone()).unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds,
        }) => {
            assert_eq!(contract_addr, &"prism0001".to_string());
            assert_eq!(
                msg,
                &to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0001".to_string(),
                    amount: Uint128::from(1_000_000u64),
                })
                .unwrap(),
            );
            assert_eq!(funds, &[])
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };
}

#[test]
fn proper_withdraw_tokens2() {
    let mut deps = mock_dependencies(&[]);
    init(&mut deps);
    post_init(&mut deps);

    let mut info1 = mock_info("addr0001", &[]);
    let mut info2 = mock_info("addr0002", &[]);
    let mut env = mock_env();

    // successful deposit
    info1.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info1.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    info2.funds = vec![Coin::new(5_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info2.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    // fast forward past phase 2
    env.block.time = env.block.time.plus_seconds(100 + SECONDS_PER_HOUR);

    // admin releases tokens, now users can claim
    let owner_info = mock_info("owner0001", &[]);
    release_tokens(deps.as_mut(), env.clone(), owner_info).unwrap();

    // addr0001 gets 1M * 1/6
    let res = withdraw_tokens(deps.as_mut(), env.clone(), info1.clone()).unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds,
        }) => {
            assert_eq!(contract_addr, &"prism0001".to_string());
            assert_eq!(
                msg,
                &to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0001".to_string(),
                    amount: Uint128::from(1_000_000u64).multiply_ratio(1u128, 6u128),
                })
                .unwrap(),
            );
            assert_eq!(funds, &[])
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };

    // addr0001 try to withdraw again, expect error
    let err = withdraw_tokens(deps.as_mut(), env.clone(), info1.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidWithdrawTokens {
            reason: "tokens were already claimed".to_string()
        }
    );

    // addr0002 gets 1M * 5/6
    let res = withdraw_tokens(deps.as_mut(), env.clone(), info2.clone()).unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg = &res.messages[0].msg;
    match msg {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds,
        }) => {
            assert_eq!(contract_addr, &"prism0001".to_string());
            assert_eq!(
                msg,
                &to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0002".to_string(),
                    amount: Uint128::from(1_000_000u64).multiply_ratio(5u128, 6u128),
                })
                .unwrap(),
            );
            assert_eq!(funds, &[])
        }
        _ => panic!("Unexpected message: {:?}", msg),
    };
}

#[test]
fn proper_admin_withdraw() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::new(1000000u128))],
    );
    init(&mut deps);
    post_init(&mut deps);

    let mut info1 = mock_info("addr0001", &[]);
    let mut info2 = mock_info("addr0002", &[]);
    let mut env = mock_env();

    // successful deposits -- total 6,000 uusd
    info1.funds = vec![Coin::new(1_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info1.clone());
    assert_eq!(res.unwrap().messages.len(), 0);

    info2.funds = vec![Coin::new(5_000, "uusd")];
    let res = deposit(deps.as_mut(), env.clone(), info2);
    assert_eq!(res.unwrap().messages.len(), 0);

    // update contract balance after deposits
    deps.querier
        .with_native_balances(&[(MOCK_CONTRACT_ADDR.to_string(), Coin::new(6_000, "uusd"))]);

    // admin tries to withdraw before phase 3, expect error
    let owner_info = mock_info("owner0001", &[]);
    let err = admin_withdraw(deps.as_mut(), env.clone(), owner_info.clone()).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidAdminWithdraw {
            reason: "cannot withdraw funds yet".to_string()
        }
    );

    // fast forward past phase 2
    env.block.time = env.block.time.plus_seconds(100 + SECONDS_PER_HOUR);

    // now admin can withdraw uusd -- unauthorized attempt
    let err = admin_withdraw(deps.as_mut(), env.clone(), info1).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // valid attempt
    let res = admin_withdraw(deps.as_mut(), env.clone(), owner_info.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "admin_withdraw"),
            attr("withdraw_amount", "6000"),
            attr("tax_amount", "60"), // 1% * 6000
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "receiver0000".to_string(), // receiver address specified on contract instantiation
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(5_940u128), // 6000 - 1% tax
            }],
        }))]
    );

    // check that users can not claim yet, even after withdraw admin
    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "addr0001".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(1_000u128),
            total_deposit: Uint128::from(6_000u128),
            withdrawable_amount: Uint128::zero(), // can not withdraw on phase 3
            tokens_to_claim: Uint128::from(166666u128), // 1000000 * 1000 / 6000
            can_claim: false,                     // tokens not released, cant claim tokens yet
        }
    );

    // admin releases tokens, now users can claim
    release_tokens(deps.as_mut(), env.clone(), owner_info).unwrap();

    let deposit_info = query_deposit_info(deps.as_ref(), env.clone(), "addr0001".to_string());
    assert_eq!(
        deposit_info.unwrap(),
        DepositResponse {
            deposit: Uint128::from(1_000u128),
            total_deposit: Uint128::from(6_000u128),
            withdrawable_amount: Uint128::zero(), // can not withdraw on phase 3
            tokens_to_claim: Uint128::from(166666u128), // 1000000 * 1000 / 6000
            can_claim: true,                      // now users can claim tokens
        }
    );
}
