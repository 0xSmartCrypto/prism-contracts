#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};

use prism_protocol::lp_vault_factory::{
    AstroConfig, Config, ExecuteMsg, InstantiateMsg, LPContracts, QueryMsg,
};

use crate::create::{create_astroport_vault, create_terraswap_vault};
use crate::error::{ContractError, ContractResult};
use crate::query::{query_config, query_vault};
use crate::state::{ASTRO_CONFIG, CONFIG, TEMP_LP_INFO, VAULTS};

use cw2::set_contract_version;
use terra_cosmwasm::TerraMsgWrapper;

const CONTRACT_NAME: &str = "prism-lp-vault-factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let cfg = Config {
        owner: info.sender,
        gov: deps.api.addr_validate(&msg.gov)?,
        prism_yasset_pair: deps.api.addr_validate(&msg.prism_yasset_pair)?,
        collector: deps.api.addr_validate(&msg.collector)?,
        fee: msg.fee,
        token_code_id: msg.token_code_id,
        yasset_contract_id: msg.yasset_contract_id,
        yasset_x_contract_id: msg.yasset_x_contract_id,
        reward_dist_contract_id: msg.reward_dist_contract_id,
    };
    CONFIG.save(deps.storage, &cfg)?;

    let astro_cfg = AstroConfig {
        lp_astro_vault_id: msg.lp_astro_vault_id,
        generator: deps.api.addr_validate(&msg.generator)?,
        factory: deps.api.addr_validate(&msg.factory)?,
    };
    ASTRO_CONFIG.save(deps.storage, &astro_cfg)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response<TerraMsgWrapper>> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            gov,
            prism_yasset_pair,
            collector,
            fee,
            token_code_id,
            yasset_contract_id,
            yasset_x_contract_id,
            reward_dist_contract_id,
        } => update_config(
            deps,
            info,
            owner,
            gov,
            prism_yasset_pair,
            collector,
            fee,
            token_code_id,
            yasset_contract_id,
            yasset_x_contract_id,
            reward_dist_contract_id,
        ),

        ExecuteMsg::CreateNewVault { amm, lp } => create_new_vault(deps, env, info, amm, lp),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Vault { amm, lp } => to_binary(&query_vault(deps, amm, &lp)?),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<Addr>,
    gov: Option<Addr>,
    prism_yasset_pair: Option<Addr>,
    collector: Option<Addr>,
    fee: Option<Decimal>,
    token_code_id: Option<u64>,
    yasset_contract: Option<u64>,
    yasset_x_contract: Option<u64>,
    reward_dist_contract: Option<u64>,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    cfg.owner = owner.unwrap_or(cfg.owner);
    cfg.gov = gov.unwrap_or(cfg.gov);
    cfg.prism_yasset_pair = prism_yasset_pair.unwrap_or(cfg.prism_yasset_pair);
    cfg.collector = collector.unwrap_or(cfg.collector);
    cfg.fee = fee.unwrap_or(cfg.fee);
    cfg.token_code_id = token_code_id.unwrap_or(cfg.token_code_id);
    cfg.yasset_contract_id = yasset_contract.unwrap_or(cfg.yasset_contract_id);
    cfg.yasset_x_contract_id = yasset_x_contract.unwrap_or(cfg.yasset_x_contract_id);
    cfg.reward_dist_contract_id = reward_dist_contract.unwrap_or(cfg.reward_dist_contract_id);
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn create_new_vault(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amm: u64,
    lp: Addr,
) -> ContractResult<Response<TerraMsgWrapper>> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    if VAULTS.load(deps.storage, (amm.into(), &lp)).is_ok() {
        return Err(ContractError::AlreadyExists {});
    }

    let new_lp = LPContracts {
        amm,
        lp: lp.clone(),
        clp_contract: Addr::unchecked(""),
        plp_contract: Addr::unchecked(""),
        ylp_contract: Addr::unchecked(""),
        collector: Addr::unchecked(""),
        yasset_contract: Addr::unchecked(""),
        yasset_x_contract: Addr::unchecked(""),
        reward_dist_contract: Addr::unchecked(""),
        vault: Addr::unchecked(""),
    };

    // create temp lp contract struct
    TEMP_LP_INFO.save(deps.storage, &new_lp)?;

    // match to the correct amm's instantiation protocol
    match amm {
        1 => create_astroport_vault(deps, env, lp),
        2 => create_terraswap_vault(deps, lp),
        _ => Err(ContractError::AmmNotSupported {}),
    }
}
