use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::state::{Config, CONFIG};
use astroport::asset::addr_validate_to_lower;
use astroport::generator_proxy::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use cw2::set_contract_version;
use prism_protocol::lp_staking::{
    Cw20HookMsg as PrismStakingCw20HookMsg, ExecuteMsg as PrismStakingExecuteMsg,
    QueryMsg as PrismStakingQueryMsg, StakerInfoResponse,
};

// version info for migration info
const CONTRACT_NAME: &str = "prism-astro-generator-proxy";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        generator_contract_addr: addr_validate_to_lower(deps.api, &msg.generator_contract_addr)?,
        pair_addr: addr_validate_to_lower(deps.api, &msg.pair_addr)?,
        lp_token_addr: addr_validate_to_lower(deps.api, &msg.lp_token_addr)?,
        reward_contract_addr: addr_validate_to_lower(deps.api, &msg.reward_contract_addr)?,
        reward_token_addr: addr_validate_to_lower(deps.api, &msg.reward_token_addr)?,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateRewards {} => update_rewards(deps),
        ExecuteMsg::SendRewards { account, amount } => send_rewards(deps, info, account, amount),
        ExecuteMsg::Withdraw { account, amount } => withdraw(deps, env, info, account, amount),
        ExecuteMsg::EmergencyWithdraw { account, amount } => {
            withdraw(deps, env, info, account, amount)
        }
        ExecuteMsg::Callback(msg) => handle_callback(deps, env, info, msg),
    }
}

fn receive_cw20(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;

    if let Ok(Cw20HookMsg::Deposit {}) = from_binary(&cw20_msg.msg) {
        if cw20_msg.sender != cfg.generator_contract_addr || info.sender != cfg.lp_token_addr {
            return Err(ContractError::Unauthorized {});
        }

        // deposit to the end reward contract here
        response
            .messages
            .push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.lp_token_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: cfg.reward_contract_addr.to_string(),
                    amount: cw20_msg.amount,
                    msg: to_binary(&PrismStakingCw20HookMsg::Bond {})?,
                })?,
            })));
    } else {
        return Err(ContractError::IncorrectCw20HookMessageVariant {});
    }
    Ok(response)
}

fn update_rewards(deps: DepsMut) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;

    response
        .messages
        .push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.reward_contract_addr.to_string(),
            funds: vec![],
            msg: to_binary(&PrismStakingExecuteMsg::ClaimRewards {
                staking_token: Some(cfg.lp_token_addr.to_string()),
            })?,
        })));

    Ok(response)
}

fn send_rewards(
    deps: DepsMut,
    info: MessageInfo,
    account: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.generator_contract_addr {
        return Err(ContractError::Unauthorized {});
    };

    response
        .messages
        .push(SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.reward_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: account.to_string(),
                amount,
            })?,
            funds: vec![],
        })));
    Ok(response)
}

fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    account: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut response = Response::new();
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.generator_contract_addr {
        return Err(ContractError::Unauthorized {});
    };

    let prev_lp_balance = {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            &cfg.lp_token_addr,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        res.balance
    };

    // withdraw from the end reward contract here
    response.messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: cfg.reward_contract_addr.to_string(),
        funds: vec![],
        msg: to_binary(&PrismStakingExecuteMsg::Unbond {
            staking_token: cfg.lp_token_addr.to_string(),
            amount: Some(amount),
        })?,
    }));

    response.messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        funds: vec![],
        msg: to_binary(&ExecuteMsg::Callback(
            CallbackMsg::TransferLpTokensAfterWithdraw {
                account,
                prev_lp_balance,
            },
        ))?,
    }));

    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let cfg = CONFIG.load(deps.storage)?;
    match msg {
        QueryMsg::Config {} => to_binary(&ConfigResponse {
            generator_contract_addr: cfg.generator_contract_addr.to_string(),
            pair_addr: cfg.pair_addr.to_string(),
            lp_token_addr: cfg.lp_token_addr.to_string(),
            reward_contract_addr: cfg.reward_contract_addr.to_string(),
            reward_token_addr: cfg.reward_token_addr.to_string(),
        }),
        QueryMsg::Deposit {} => {
            // query the end reward contract for total deposit amount if possible or implement storing local balance
            let res: StdResult<StakerInfoResponse> = deps.querier.query_wasm_smart(
                cfg.reward_contract_addr,
                &PrismStakingQueryMsg::StakerInfo {
                    staker: env.contract.address.to_string(),
                    staking_token: Some(cfg.lp_token_addr.to_string()),
                },
            );
            let reward_infos = res?.reward_infos;
            let deposit_amount = if !reward_infos.is_empty() {
                reward_infos[0].bond_amount
            } else {
                Uint128::zero()
            };

            to_binary(&deposit_amount)
        }
        QueryMsg::Reward {} => {
            let res: Result<BalanceResponse, StdError> = deps.querier.query_wasm_smart(
                cfg.reward_token_addr,
                &Cw20QueryMsg::Balance {
                    address: env.contract.address.into_string(),
                },
            );
            let reward_amount = res?.balance;

            to_binary(&reward_amount)
        }
        QueryMsg::PendingToken {} => {
            // query the end reward contract for pending token amount if possible
            let res: StdResult<StakerInfoResponse> = deps.querier.query_wasm_smart(
                cfg.reward_contract_addr,
                &PrismStakingQueryMsg::StakerInfo {
                    staker: env.contract.address.to_string(),
                    staking_token: Some(cfg.lp_token_addr.to_string()),
                },
            );
            let reward_infos = res?.reward_infos;
            let pending_reward = if !reward_infos.is_empty() {
                reward_infos[0].pending_reward
            } else {
                Uint128::zero()
            };

            to_binary(&Some(pending_reward))
        }
        QueryMsg::RewardInfo {} => {
            let config = CONFIG.load(deps.storage)?;
            to_binary(&config.reward_token_addr)
        }
    }
}

pub fn handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response, ContractError> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    match msg {
        CallbackMsg::TransferLpTokensAfterWithdraw {
            account,
            prev_lp_balance,
        } => transfer_lp_tokens_after_withdraw(deps, env, account, prev_lp_balance),
    }
}

pub fn transfer_lp_tokens_after_withdraw(
    deps: DepsMut,
    env: Env,
    account: Addr,
    prev_lp_balance: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let amount = {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            &cfg.lp_token_addr,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        res.balance - prev_lp_balance
    };

    Ok(Response::new().add_message(WasmMsg::Execute {
        contract_addr: cfg.lp_token_addr.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: account.to_string(),
            amount,
        })?,
    }))
}
