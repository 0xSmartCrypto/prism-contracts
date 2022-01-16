#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128,
};

use cw20::Cw20ReceiveMsg;
use prism_protocol::astroport_lp_vault::{
    Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, LPInfo, QueryMsg,
};

use crate::bond::{bond, unbond};
use crate::error::{ContractError, ContractResult};
use crate::query::{query_config, query_generator_rewards_info, query_pair_info};
use crate::refract::{merge, split};
use crate::state::{CONFIG, LP_INFO};

use cw2::set_contract_version;

const CONTRACT_NAME: &str = "prism-astroport-lp-vault";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let data = Config {
        owner: info.sender,
        generator: deps.api.addr_validate(&msg.generator)?,
        factory: deps.api.addr_validate(&msg.factory)?,
        reward_dist: Addr::unchecked(""),
        fee: msg.fee,
    };
    CONFIG.save(deps.storage, &data)?;

    // Get relevant info to create new LP token set
    let token = deps.api.addr_validate(&msg.lp_contract)?;
    let pair_info = query_pair_info(deps.as_ref(), &deps.querier, token.clone())?;
    let generator_rewards_info = query_generator_rewards_info(deps.as_ref(), &deps.querier)?;

    let lp_info = LPInfo {
        pair_asset_info: pair_info.asset_infos.clone(),
        generator_reward_info: generator_rewards_info,
        amt_lp: Uint128::zero(),
        amt_clp: Uint128::zero(),
        last_liquidity: Decimal::zero(),
        pair_contract: pair_info.contract_addr,
        lp_contract: token,
        clp_contract: deps.api.addr_validate(&msg.clp_contract)?,
        plp_contract: deps.api.addr_validate(&msg.plp_contract)?,
        ylp_contract: deps.api.addr_validate(&msg.ylp_contract)?,
    };
    LP_INFO.save(deps.storage, &lp_info)?;
    Ok(Response::new().add_attributes(vec![attr("action", "instantiate")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        // owner function
        ExecuteMsg::UpdateConfig {
            owner,
            generator,
            factory,
            reward_dist,
            fee,
        } => update_config(deps, info, owner, generator, factory, reward_dist, fee),

        // user functions
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info.sender, amount),
        ExecuteMsg::Merge { amount } => merge(deps, info, amount),
        ExecuteMsg::Split { amount } => split(deps, info, amount),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> ContractResult<Response> {
    let cw20_sender: Addr = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond {} => bond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        // provide query for individual StakerInfo
        // provide query for all StakerInfo for an individual user
        // bonded amount info
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<Addr>,
    generator: Option<Addr>,
    factory: Option<Addr>,
    reward_dist: Option<Addr>,
    fee: Option<Decimal>,
) -> ContractResult<Response> {
    let mut conf = CONFIG.load(deps.storage)?;
    if info.sender.as_str() != conf.owner {
        return Err(ContractError::Unauthorized {});
    }

    conf.owner = owner.unwrap_or(conf.owner);
    conf.generator = generator.unwrap_or(conf.generator);
    conf.factory = factory.unwrap_or(conf.factory);
    conf.reward_dist = reward_dist.unwrap_or(conf.reward_dist);
    conf.fee = fee.unwrap_or(conf.fee);
    CONFIG.save(deps.storage, &conf)?;
    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}
