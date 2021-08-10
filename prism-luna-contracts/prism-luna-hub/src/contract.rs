use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StakingMsg, StdError, StdResult, SubMsg, Uint128,
};

use crate::error::ContractError;
use crate::msg::{
    CountResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, MyCountResponse, QueryMsg,
};
use crate::state::{MyState, Parameters, State, MY_STATE, PARAMETERS, STATE};

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
        ExecuteMsg::UpdateParameters { token_contract } => {
            try_update_parameters(deps, info, token_contract)
        }
        ExecuteMsg::Bond { validator } => try_bond(deps, info, validator),
    }
}

pub fn try_bond(
    deps: DepsMut,
    info: MessageInfo,
    validator: String,
) -> Result<Response, ContractError> {
    // only allow one denom to be sent to the contract
    if info.funds.len() != 1usize {
        return Err(ContractError::Std(StdError::generic_err(
            "You must only send one type of coin to this contract.",
        )));
    }

    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == "uluna" && x.amount > Uint128::zero())
        .ok_or_else(|| StdError::generic_err("No uluna has been provided."))?;

    let messages = vec![
        // send the delegation message
        CosmosMsg::Staking(StakingMsg::Delegate {
            validator,
            amount: payment.clone(),
        }),
    ];

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "mint"),
        attr("from", info.sender),
        attr("bonded", payment.amount),
    ]))
}

pub fn try_update_parameters(
    deps: DepsMut,
    info: MessageInfo,
    token_contract: Option<Addr>,
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
        Err(_) => Ok(MyCountResponse {
            addr,
            count: Uint128::from(0u128),
        }),
        Ok(r) => Ok(MyCountResponse {
            addr,
            count: r.count,
        }),
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
