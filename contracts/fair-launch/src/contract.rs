use crate::error::ContractError;
use crate::state::{Config, CONFIG, DEPOSITS, TOTAL_DEPOSIT};

use cosmwasm_std::{
    entry_point, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::fair_launch::{
    DepositResponse, ExecuteMsg, InstantiateMsg, LaunchConfig, QueryMsg,
};
use std::cmp::min;
use terraswap::asset::{Asset, AssetInfo};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let cfg = Config {
        owner: msg.owner,
        token: msg.token,
        launch_config: None,
    };
    TOTAL_DEPOSIT.save(deps.storage, &Uint128::zero())?;
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => deposit(deps, env, info),
        ExecuteMsg::Withdraw { amount } => withdraw(deps, env, info, amount),
        ExecuteMsg::WithdrawTokens {} => withdraw_tokens(deps, env, info),
        ExecuteMsg::PostInitialize { launch_config } => {
            post_initialize(deps, env, info, launch_config)
        }
    }
}

pub fn post_initialize(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    launch_config: LaunchConfig,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != cfg.owner.as_str() {
        return Err(ContractError::Unauthorized {});
    }

    if cfg.launch_config.is_some() {
        return Err(ContractError::DuplicatePostInit {});
    }

    if env.block.time.seconds() > launch_config.phase1_start
        || launch_config.phase1_start > launch_config.phase2_start
        || launch_config.phase2_end > launch_config.phase2_end
    {
        return Err(ContractError::InvalidLaunchConfig {});
    }

    cfg.launch_config = Some(launch_config.clone());

    CONFIG.save(deps.storage, &cfg)?;

    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.token.clone(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount: launch_config.amount.clone(),
            })?,
            funds: vec![],
        })),
    )
}

pub fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?.launch_config.unwrap();
    if env.block.time.seconds() >= cfg.phase2_start {
        return Err(ContractError::InvalidDeposit {
            reason: "deposit period is over".to_string(),
        });
    }

    if info.funds.len() != 1 {
        return Err(ContractError::InvalidDeposit {
            reason: "requires 1 coin deposited".to_string(),
        });
    }
    let coin = &info.funds[0];
    if coin.denom != "uusd".to_string() || coin.amount == Uint128::zero() {
        return Err(ContractError::InvalidDeposit {
            reason: "requires uusd and positive amount".to_string(),
        });
    }

    let canon_addr = deps.api.addr_canonicalize(info.sender.as_str())?;
    DEPOSITS.update(
        deps.storage,
        canon_addr.as_slice(),
        |curr| -> StdResult<Uint128> { Ok(curr.unwrap_or(Uint128::zero()) + coin.amount) },
    )?;
    TOTAL_DEPOSIT.update(deps.storage, |curr| -> StdResult<Uint128> {
        Ok(curr + coin.amount)
    })?;

    Ok(Response::new())
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?.launch_config.unwrap();
    if env.block.time.seconds() >= cfg.phase2_end {
        return Err(ContractError::InvalidWithdraw {
            reason: "withdraw period is over".to_string(),
        });
    }
    let canon_addr = deps.api.addr_canonicalize(info.sender.as_str())?;
    let cur_deposit = DEPOSITS
        .load(deps.storage, canon_addr.as_slice())
        .unwrap_or(Uint128::zero());

    // withdraw everything if amount > deposit amount, possibly error instead?
    let withdraw_amount = min(cur_deposit, amount);
    if withdraw_amount == Uint128::zero() {
        return Err(ContractError::InvalidWithdraw {
            reason: "no funds available to withdraw".to_string(),
        });
    }

    let withdraw_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        amount: withdraw_amount,
    };

    DEPOSITS.save(
        deps.storage,
        canon_addr.as_slice(),
        &(cur_deposit - withdraw_asset.amount),
    )?;

    TOTAL_DEPOSIT.update(deps.storage, |curr| -> StdResult<Uint128> {
        Ok(curr - withdraw_asset.amount)
    })?;

    let msg = withdraw_asset.into_msg(&deps.querier, info.sender)?;
    Ok(Response::new().add_message(msg))
}

pub fn withdraw_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let launch_cfg = cfg.launch_config.unwrap();

    if env.block.time.seconds() < launch_cfg.phase2_end {
        return Err(ContractError::InvalidWithdrawTokens {
            reason: "cannot withdraw tokens yet".to_string(),
        });
    }

    let canon_addr = deps.api.addr_canonicalize(info.sender.as_str())?;
    let deposited = DEPOSITS.load(deps.storage, canon_addr.as_slice())?;
    let deposit_total = TOTAL_DEPOSIT.load(deps.storage)?;
    let amount = launch_cfg.amount.multiply_ratio(deposited, deposit_total);
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidWithdrawTokens {
            reason: "no tokens available for withdraw".to_string(),
        });
    }

    DEPOSITS.save(deps.storage, canon_addr.as_slice(), &Uint128::zero())?;
    let to_send = Asset {
        info: AssetInfo::Token {
            contract_addr: cfg.token,
        },
        amount,
    };
    Ok(Response::new().add_message(to_send.into_msg(&deps.querier, info.sender)?))
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn query_deposit_info(deps: Deps, address: String) -> StdResult<DepositResponse> {
    let canon_addr = deps.api.addr_canonicalize(&address)?;
    Ok(DepositResponse {
        address_deposit: DEPOSITS
            .load(deps.storage, canon_addr.as_slice())
            .unwrap_or(Uint128::zero()),
        total_deposit: TOTAL_DEPOSIT.load(deps.storage)?,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DepositInfo { address } => to_binary(&query_deposit_info(deps, address)?),
    }
}
