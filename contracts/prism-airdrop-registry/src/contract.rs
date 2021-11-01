#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg,
};
use serde::{Deserialize, Serialize};

use crate::state::{
    read_airdrop_info, read_all_airdrop_infos, read_config, remove_airdrop_info,
    store_airdrop_info, store_config, update_airdrop_info, Config, CONFIG,
};
use prism_protocol::airdrop::{
    AirdropInfo, AirdropInfoElem, AirdropInfoResponse, ClaimType, ConfigResponse, ExecuteMsg,
    InstantiateMsg, QueryMsg,
};
use prism_protocol::vault::ExecuteMsg as VaultHandleMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let sndr_raw = deps.api.addr_canonicalize(&info.sender.to_string())?;

    let config = Config {
        owner: sndr_raw,
        vault_contract: msg.vault_contract,
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

    let airdrop_info = read_airdrop_info(deps.storage, airdrop_token.clone())
        .map_err(|_| StdError::generic_err("no info registered for this airdrop token"))?;
    let claim_msg: Binary = match airdrop_info.claim_type {
        ClaimType::Generic => to_binary(&GenericAirdropExecuteMsg::Claim {
            stage,
            amount,
            proof,
        })?,
    };

    let vault_claim_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.vault_contract,
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
    let sender_raw = deps.api.addr_canonicalize(&info.sender.to_string())?;
    if sender_raw != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(o) = owner {
        let owner_raw = deps.api.addr_canonicalize(&o)?;
        config.owner = owner_raw
    }

    if let Some(vault) = vault_contract {
        config.vault_contract = vault;
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
    let sender_raw = deps.api.addr_canonicalize(&info.sender.to_string())?;
    if sender_raw != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let exists = read_airdrop_info(deps.storage, airdrop_token.clone());
    if exists.is_ok() {
        return Err(StdError::generic_err(format!(
            "There is a token info with this {}",
            airdrop_token
        )));
    }

    CONFIG.update(deps.storage, |mut conf| -> StdResult<Config> {
        conf.airdrop_tokens.push(airdrop_token.clone());
        Ok(conf)
    })?;

    store_airdrop_info(deps.storage, airdrop_token.clone(), airdrop_info)?;

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
    let sender_raw = deps.api.addr_canonicalize(&info.sender.to_string())?;
    if sender_raw != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let exists = read_airdrop_info(deps.storage, airdrop_token.clone());
    if exists.is_err() {
        return Err(StdError::generic_err(format!(
            "There is no token info with this {}",
            airdrop_token
        )));
    }

    update_airdrop_info(deps.storage, airdrop_token.clone(), airdrop_info)?;
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
    let sender_raw = deps.api.addr_canonicalize(&info.sender.to_string())?;
    if sender_raw != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let exists = read_airdrop_info(deps.storage, airdrop_token.clone());
    if exists.is_err() {
        return Err(StdError::generic_err(format!(
            "There is no token info with this {}",
            airdrop_token
        )));
    }

    CONFIG.update(deps.storage, |mut conf| -> StdResult<Config> {
        conf.airdrop_tokens.retain(|item| item != &airdrop_token);
        Ok(conf)
    })?;

    remove_airdrop_info(deps.storage, airdrop_token.clone())?;
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
    let owner_addr = deps.api.addr_humanize(&config.owner)?;

    Ok(ConfigResponse {
        owner: owner_addr.to_string(),
        vault_contract: config.vault_contract,
        airdrop_tokens: config.airdrop_tokens,
    })
}

fn query_airdrop_infos(
    deps: Deps,
    airdrop_token: Option<String>,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<AirdropInfoResponse> {
    if let Some(air_token) = airdrop_token {
        let info = read_airdrop_info(deps.storage, air_token.clone())?;

        Ok(AirdropInfoResponse {
            airdrop_info: vec![AirdropInfoElem {
                airdrop_token: air_token,
                info,
            }],
        })
    } else {
        let infos = read_all_airdrop_infos(deps.storage, start_after, limit)?;
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
