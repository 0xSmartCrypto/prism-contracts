use crate::error::ContractError;
use crate::state::{Config, CONFIG, DEPOSITS, TOTAL_DEPOSIT};

use astroport::asset::{Asset, AssetInfo};
use astroport::querier::query_balance;
use cosmwasm_std::{
    attr, entry_point, to_binary, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use prism_protocol::fair_launch::{
    ConfigResponse, DepositResponse, ExecuteMsg, InstantiateMsg, LaunchConfig, QueryMsg,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.withdraw_fee > Decimal::one() {
        return Err(ContractError::InvalidFee {});
    }
    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        token: deps.api.addr_validate(&msg.token)?,
        launch_config: None,
        base_denom: msg.base_denom,
        withdraw_threshold: msg.withdraw_threshold,
        withdraw_fee: msg.withdraw_fee,
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
        ExecuteMsg::AdminWithdraw {} => admin_withdraw(deps, env, info),
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
        || launch_config.phase2_start > launch_config.phase2_end
    {
        return Err(ContractError::InvalidLaunchConfig {});
    }

    cfg.launch_config = Some(launch_config.clone());

    CONFIG.save(deps.storage, &cfg)?;

    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.token.to_string(),
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
    let cfg = CONFIG.load(deps.storage)?;
    let launch_cfg = cfg.launch_config.unwrap();
    if env.block.time.seconds() >= launch_cfg.phase2_start {
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
    if coin.denom != cfg.base_denom || coin.amount == Uint128::zero() {
        return Err(ContractError::InvalidDeposit {
            reason: format!("requires {} and positive amount", cfg.base_denom),
        });
    }

    DEPOSITS.update(deps.storage, &info.sender, |curr| -> StdResult<Uint128> {
        Ok(curr.unwrap_or(Uint128::zero()) + coin.amount)
    })?;
    TOTAL_DEPOSIT.update(deps.storage, |curr| -> StdResult<Uint128> {
        Ok(curr + coin.amount)
    })?;

    Ok(Response::new().add_attribute("action", "deposit"))
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let launch_config = cfg.launch_config.unwrap();
    let current_time = env.block.time.seconds();

    if current_time >= launch_config.phase2_end {
        return Err(ContractError::InvalidWithdraw {
            reason: "withdraw period is over".to_string(),
        });
    }
    let cur_deposit = DEPOSITS
        .load(deps.storage, &info.sender)
        .unwrap_or(Uint128::zero());

    if cur_deposit == Uint128::zero() {
        return Err(ContractError::InvalidWithdraw {
            reason: "no funds available to withdraw".to_string(),
        });
    }

    let withdraw_amount = match amount {
        None => cur_deposit,
        Some(requested_amount) => {
            if requested_amount > cur_deposit {
                return Err(ContractError::InvalidWithdraw {
                    reason: format!(
                        "can not withdraw more than current deposit amount ({})",
                        cur_deposit
                    ),
                });
            }
            if requested_amount == Uint128::zero() {
                return Err(ContractError::InvalidWithdraw {
                    reason: "withdraw amount must be bigger than 0".to_string(),
                });
            }

            requested_amount
        }
    };

    DEPOSITS.save(deps.storage, &info.sender, &(cur_deposit - withdraw_amount))?;

    TOTAL_DEPOSIT.update(deps.storage, |curr| -> StdResult<Uint128> {
        Ok(curr - withdraw_amount)
    })?;

    let mut withdraw_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: cfg.base_denom,
        },
        amount: withdraw_amount,
    };

    // to prevent massive withdraw on phase 2, charge fee
    let withdraw_fee_amount =
        if current_time > launch_config.phase2_start && withdraw_amount > cfg.withdraw_threshold {
            let withdraw_fee_amount = withdraw_amount * cfg.withdraw_fee;
            withdraw_asset.amount -= withdraw_fee_amount;

            withdraw_fee_amount
        } else {
            Uint128::zero()
        };

    let tax_amount = withdraw_asset.compute_tax(&deps.querier)?;
    let msg = withdraw_asset
        .clone()
        .into_msg(&deps.querier, info.sender)?;
    Ok(Response::new().add_message(msg).add_attributes(vec![
        attr("action", "withdraw"),
        attr("withdraw_amount", withdraw_asset.amount.to_string()),
        attr("withdraw_fee", withdraw_fee_amount.to_string()),
        attr("tax_amount", tax_amount.to_string()),
    ]))
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

    let deposited = DEPOSITS.load(deps.storage, &info.sender)?;
    let deposit_total = TOTAL_DEPOSIT.load(deps.storage)?;
    let amount = launch_cfg.amount.multiply_ratio(deposited, deposit_total);
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidWithdrawTokens {
            reason: "no tokens available for withdraw".to_string(),
        });
    }

    DEPOSITS.save(deps.storage, &info.sender, &Uint128::zero())?;
    let to_send = Asset {
        info: AssetInfo::Token {
            contract_addr: cfg.token,
        },
        amount,
    };
    Ok(Response::new()
        .add_message(to_send.into_msg(&deps.querier, info.sender)?)
        .add_attributes(vec![
            attr("action", "withdraw_tokens"),
            attr("withdraw_amount", amount.to_string()),
        ]))
}

pub fn admin_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let launch_cfg = cfg.launch_config.unwrap();

    if info.sender.as_str() != cfg.owner.as_str() {
        return Err(ContractError::Unauthorized {});
    }

    if env.block.time.seconds() < launch_cfg.phase2_end {
        return Err(ContractError::InvalidAdminWithdraw {
            reason: "cannot withdraw funds yet".to_string(),
        });
    }

    let balance = query_balance(&deps.querier, env.contract.address, cfg.base_denom.clone())?;

    let withdraw_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: cfg.base_denom,
        },
        amount: balance,
    };

    let tax_amount = withdraw_asset.compute_tax(&deps.querier)?;
    let msg = withdraw_asset.into_msg(&deps.querier, info.sender)?;
    Ok(Response::new().add_message(msg).add_attributes(vec![
        attr("action", "admin_withdraw"),
        attr("withdraw_amount", balance.to_string()),
        attr("tax_amount", tax_amount.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DepositInfo { address } => to_binary(&query_deposit_info(deps, address)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    cfg.as_res()
}

pub fn query_deposit_info(deps: Deps, address: String) -> StdResult<DepositResponse> {
    let addr = deps.api.addr_validate(&address)?;

    Ok(DepositResponse {
        address_deposit: DEPOSITS
            .load(deps.storage, &addr)
            .unwrap_or(Uint128::zero()),
        total_deposit: TOTAL_DEPOSIT.load(deps.storage)?,
    })
}
