use std::ops::Add;

use cosmwasm_std::{Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, entry_point, to_binary, Uint128};

use crate::error::ContractError;
use crate::msg::{CountResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, MyCountResponse, QueryMsg};
use crate::state::{MY_STATE, STATE, State, MyState, PARAMETERS, Parameters};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let parameters = Parameters {
        owner: info.sender.clone(),
        token_contract: None, 
    };
    PARAMETERS.save(deps.storage, &parameters)?;

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateParameters {
            token_contract
        } => try_update_parameters(deps, info, token_contract),
    }
}

pub fn try_update_parameters(
    deps: DepsMut,
    info: MessageInfo,
    token_contract: Option<Addr>
) -> Result<Response, ContractError> {
    PARAMETERS.update(deps.storage, |mut params| -> Result<_, ContractError> {
        // deny if not owner
        if info.sender != params.owner {
            return Err(ContractError::Unauthorized {});
        }

        if token_contract.is_some() {
            params.token_contract = token_contract;
        }
        Ok(params)
    })?;

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query_count(deps)?),
        QueryMsg::GetMyCount { addr } => to_binary(&my_query_count(deps, addr)?),
    }
}

fn query_count(deps: Deps) -> StdResult<CountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(CountResponse { count: state.count })
}

fn my_query_count(deps: Deps, addr: Addr) -> StdResult<MyCountResponse> {
    let state = MY_STATE.load(deps.storage, addr.as_str().as_bytes());
    match state {
        Err(_) => Ok(MyCountResponse { addr, count: Uint128::from(0u128) }),
        Ok(r) => Ok(MyCountResponse { addr, count: r.count })
    }
}

#[entry_point]
pub fn migrate(_: DepsMut, _: Env, _: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};
}
