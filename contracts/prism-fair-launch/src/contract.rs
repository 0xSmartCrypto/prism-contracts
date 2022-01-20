use crate::error::ContractError;
use crate::state::{Config, DepositInfo, CONFIG, DEPOSITS, TOTAL_DEPOSIT};

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

pub const SECONDS_PER_HOUR: u64 = 60 * 60;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        token: deps.api.addr_validate(&msg.token)?,
        launch_config: None,
        base_denom: msg.base_denom,
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

    // phase 2 must be longer than 1 hour, since we are using one hour slots
    if (launch_config.phase2_end - launch_config.phase2_start) < SECONDS_PER_HOUR {
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

    DEPOSITS.update(
        deps.storage,
        &info.sender,
        |curr| -> StdResult<DepositInfo> {
            let mut deposit = curr.unwrap_or_default();
            deposit.amount += coin.amount;

            Ok(deposit)
        },
    )?;
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
    let mut deposit_info = DEPOSITS
        .load(deps.storage, &info.sender)
        .unwrap_or_default();

    if deposit_info.amount == Uint128::zero() {
        return Err(ContractError::InvalidWithdraw {
            reason: "no funds available to withdraw".to_string(),
        });
    }

    let withdrawable_amount = if current_time >= launch_config.phase2_start {
        // check if user already withdrew on phase 2
        if deposit_info.withdrew_phase2 {
            return Err(ContractError::InvalidWithdraw {
                reason: "a withdraw was already executed on phase 2".to_string(),
            });
        }

        let current_slot = (launch_config.phase2_end - current_time) / SECONDS_PER_HOUR;
        let total_slots =
            (launch_config.phase2_end - launch_config.phase2_start) / SECONDS_PER_HOUR;
        let withdrawable_portion =
            Decimal::from_ratio(current_slot, total_slots).min(Decimal::one());

        // on phase 2 can only withraw one time, so flag the position
        deposit_info.withdrew_phase2 = true;

        deposit_info.amount * withdrawable_portion
    } else {
        deposit_info.amount
    };

    let withdraw_amount = match amount {
        None => withdrawable_amount,
        Some(requested_amount) => {
            if requested_amount > withdrawable_amount {
                return Err(ContractError::InvalidWithdraw {
                    reason: format!(
                        "can not withdraw more than current withrawable amount ({})",
                        withdrawable_amount
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

    // update user deposit amount
    deposit_info.amount -= withdraw_amount;

    DEPOSITS.save(deps.storage, &info.sender, &deposit_info)?;

    TOTAL_DEPOSIT.update(deps.storage, |curr| -> StdResult<Uint128> {
        Ok(curr - withdraw_amount)
    })?;

    let withdraw_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: cfg.base_denom,
        },
        amount: withdraw_amount,
    };
    let tax_amount = withdraw_asset.compute_tax(&deps.querier)?;
    let msg = withdraw_asset
        .clone()
        .into_msg(&deps.querier, info.sender)?;

    Ok(Response::new().add_message(msg).add_attributes(vec![
        attr("action", "withdraw"),
        attr("withdraw_amount", withdraw_asset.amount.to_string()),
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

    let mut deposit_info = DEPOSITS.load(deps.storage, &info.sender).map_err(|_| {
        ContractError::InvalidWithdrawTokens {
            reason: "deposit information not found".to_string(),
        }
    })?;
    if deposit_info.tokens_claimed {
        return Err(ContractError::InvalidWithdrawTokens {
            reason: "tokens were already claimed".to_string(),
        });
    }

    let deposit_total = TOTAL_DEPOSIT.load(deps.storage)?;
    let amount = launch_cfg
        .amount
        .multiply_ratio(deposit_info.amount, deposit_total);
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidWithdrawTokens {
            reason: "no tokens available for withdraw".to_string(),
        });
    }

    // update claimed flag, we don't delete storage to keep the record
    deposit_info.tokens_claimed = true;

    DEPOSITS.save(deps.storage, &info.sender, &deposit_info)?;
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
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DepositInfo { address } => to_binary(&query_deposit_info(deps, env, address)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;

    cfg.as_res()
}

pub fn query_deposit_info(deps: Deps, env: Env, address: String) -> StdResult<DepositResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let launch_config = CONFIG.load(deps.storage)?.launch_config.unwrap();
    let deposit_info = DEPOSITS.load(deps.storage, &addr).unwrap_or_default();
    let current_time = env.block.time.seconds();

    let withdrawable_amount =
        if current_time >= launch_config.phase2_start && !deposit_info.amount.is_zero() {
            if deposit_info.withdrew_phase2 || current_time >= launch_config.phase2_end {
                Uint128::zero()
            } else {
                let current_slot = (launch_config.phase2_end - current_time) / SECONDS_PER_HOUR;
                let total_slots =
                    (launch_config.phase2_end - launch_config.phase2_start) / SECONDS_PER_HOUR;
                dbg!(current_slot);
                let withdrawable_portion =
                    Decimal::from_ratio(current_slot, total_slots).min(Decimal::one());

                deposit_info.amount * withdrawable_portion
            }
        } else {
            deposit_info.amount
        };

    let total_deposit = TOTAL_DEPOSIT.load(deps.storage)?;
    let tokens_to_claim = if !total_deposit.is_zero() {
        launch_config
            .amount
            .multiply_ratio(deposit_info.amount, total_deposit)
    } else {
        Uint128::zero()
    };

    Ok(DepositResponse {
        deposit: deposit_info.amount,
        total_deposit,
        withdrawable_amount,
        tokens_to_claim,
        can_claim: current_time >= launch_config.phase2_end
            && !tokens_to_claim.is_zero()
            && !deposit_info.tokens_claimed,
    })
}
