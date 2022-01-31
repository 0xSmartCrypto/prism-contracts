#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use serde::{Deserialize, Serialize};

use crate::state::{
    read_airdrop_info, read_all_airdrop_infos, read_config, remove_airdrop_info,
    store_airdrop_info, store_config, update_airdrop_info, Config, CONFIG,
};
use prism_protocol::airdrop_registry::{
    AirdropInfo, AirdropInfoElem, AirdropInfoResponse, ClaimType, ConfigResponse, ExecuteMsg,
    InstantiateMsg, QueryMsg,
};
use prism_protocol::vault::ExecuteMsg as VaultHandleMsg;

const CONTRACT_NAME: &str = "prism-airdrop-registry";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: info.sender,
        vault_contract: deps.api.addr_validate(&msg.vault_contract)?,
        airdrop_tokens: vec![],
    };

    store_config(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::FabricateClaim {
            airdrop_token,
            stage,
            amount,
            proof,
        } => execute_fabricate_claim(deps, airdrop_token, stage, amount, proof),
        ExecuteMsg::UpdateConfig {
            owner,
            vault_contract,
        } => execute_update_config(deps, info, owner, vault_contract),
        ExecuteMsg::AddAirdropInfo {
            airdrop_token,
            airdrop_info,
        } => execute_add_airdrop(deps, env, info, airdrop_token, airdrop_info),
        ExecuteMsg::RemoveAirdropInfo { airdrop_token } => {
            execute_remove_airdrop(deps, env, info, airdrop_token)
        }
        ExecuteMsg::UpdateAirdropInfo {
            airdrop_token,
            airdrop_info,
        } => execute_update_airdrop(deps, env, info, airdrop_token, airdrop_info),
    }
}

fn execute_fabricate_claim(
    deps: DepsMut,
    airdrop_token: String,
    stage: u8,
    amount: Uint128,
    proof: Vec<String>,
) -> StdResult<Response> {
    let config = read_config(deps.storage)?;

    let airdrop_token_addr = deps.api.addr_validate(&airdrop_token)?;
    let airdrop_info = read_airdrop_info(deps.storage, &airdrop_token_addr)
        .map_err(|_| StdError::generic_err("no info registered for this airdrop token"))?;
    let claim_msg: Binary = match airdrop_info.claim_type {
        ClaimType::Generic => to_binary(&GenericAirdropExecuteMsg::Claim {
            stage,
            amount,
            proof,
        })?,
    };

    let vault_claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.vault_contract.to_string(),
        msg: to_binary(&VaultHandleMsg::ClaimAirdrop {
            airdrop_token_contract: airdrop_token,
            airdrop_contract: airdrop_info.airdrop_contract,
            claim_msg,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(vault_claim_msg)
        .add_attributes(vec![attr("action", "fabricate_generic_claim")]))
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    vault_contract: Option<String>,
) -> StdResult<Response> {
    // only owner can send this message.
    let mut config = read_config(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(o) = owner {
        config.owner = deps.api.addr_validate(&o)?;
    }

    if let Some(vault) = vault_contract {
        config.vault_contract = deps.api.addr_validate(&vault)?;
    }

    store_config(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

pub fn execute_add_airdrop(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    airdrop_token: String,
    airdrop_info: AirdropInfo,
) -> StdResult<Response> {
    // only owner can send this message.
    let config = read_config(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let airdrop_token_addr = deps.api.addr_validate(&airdrop_token)?;
    let exists = read_airdrop_info(deps.storage, &airdrop_token_addr);
    if exists.is_ok() {
        return Err(StdError::generic_err(format!(
            "There is a token info with this {}",
            airdrop_token
        )));
    }

    let airdrop_token_addr = deps.api.addr_validate(&airdrop_token)?;
    CONFIG.update(deps.storage, |mut conf| -> StdResult<Config> {
        conf.airdrop_tokens.push(airdrop_token_addr.clone());
        Ok(conf)
    })?;

    store_airdrop_info(deps.storage, &airdrop_token_addr, &airdrop_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "add_airdrop_info"),
        attr("airdrop_token", airdrop_token),
    ]))
}

pub fn execute_update_airdrop(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    airdrop_token: String,
    airdrop_info: AirdropInfo,
) -> StdResult<Response> {
    // only owner can send this message.
    let config = read_config(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let airdrop_token_addr = deps.api.addr_validate(&airdrop_token)?;
    let exists = read_airdrop_info(deps.storage, &airdrop_token_addr);
    if exists.is_err() {
        return Err(StdError::generic_err(format!(
            "There is no token info with this {}",
            airdrop_token
        )));
    }

    update_airdrop_info(deps.storage, &airdrop_token_addr, &airdrop_info)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "update_airdrop_info"),
        attr("airdrop_token", airdrop_token),
    ]))
}

pub fn execute_remove_airdrop(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    airdrop_token: String,
) -> StdResult<Response> {
    // only owner can send this message.
    let config = read_config(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let airdrop_token_addr = deps.api.addr_validate(&airdrop_token)?;
    let exists = read_airdrop_info(deps.storage, &airdrop_token_addr);
    if exists.is_err() {
        return Err(StdError::generic_err(format!(
            "There is no token info with this {}",
            airdrop_token
        )));
    }

    CONFIG.update(deps.storage, |mut conf| -> StdResult<Config> {
        conf.airdrop_tokens
            .retain(|item| item != &airdrop_token_addr);
        Ok(conf)
    })?;

    remove_airdrop_info(deps.storage, &airdrop_token_addr)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "remove_airdrop_info"),
        attr("airdrop_token", airdrop_token),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AirdropInfo {
            airdrop_token,
            start_after,
            limit,
        } => to_binary(&query_airdrop_infos(
            deps,
            airdrop_token,
            start_after,
            limit,
        )?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = read_config(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        vault_contract: config.vault_contract.to_string(),
        airdrop_tokens: config
            .airdrop_tokens
            .iter()
            .map(|item| item.to_string())
            .collect(),
    })
}

fn query_airdrop_infos(
    deps: Deps,
    airdrop_token: Option<String>,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<AirdropInfoResponse> {
    if let Some(air_token) = airdrop_token {
        let airdrop_token_addr = deps.api.addr_validate(&air_token)?;
        let info = read_airdrop_info(deps.storage, &airdrop_token_addr)?;

        Ok(AirdropInfoResponse {
            airdrop_info: vec![AirdropInfoElem {
                airdrop_token: air_token,
                info,
            }],
        })
    } else {
        let start_after_addr: Option<Addr> =
            start_after.map(|item| deps.api.addr_validate(&item).unwrap());
        let infos = read_all_airdrop_infos(deps.storage, start_after_addr, limit)?;
        Ok(AirdropInfoResponse {
            airdrop_info: infos,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GenericAirdropExecuteMsg {
    Claim {
        stage: u8,
        amount: Uint128,
        proof: Vec<String>,
    },
}
