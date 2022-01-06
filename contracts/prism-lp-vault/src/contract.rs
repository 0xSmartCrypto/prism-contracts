#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    attr, Addr, Decimal,
};

use prism_protocol::lp_vault::{
    Cw20HookMsg, Config, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakingMode, 
};

use crate::state::{CONFIG, NUM_LPS};
use crate::query::{query_config,};
use crate::bond::{bond, unbond, mint, burn, create_tokens};
use crate::refract::{split, merge};
use crate::stake::{stake, unstake, claim_rewards, update_staking_mode, update_lp_rewards, send_staker_rewards};

use astroport::asset::AssetInfo;
use cw20::Cw20ReceiveMsg;
use terra_cosmwasm::TerraMsgWrapper;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let sender = info.sender.clone();
    let data = Config {
        owner: sender.to_string(),
        generator: msg.generator,
        factory: msg.factory,
        collector: msg.collector,
        fee: msg.fee,
    };
    CONFIG.save(deps.storage, &data)?;
    NUM_LPS.save(deps.storage, &0)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        // cw20 functions
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),

        // owner functions
        ExecuteMsg::UpdateConfig { owner, generator, factory, collector, fee } => update_config(deps, env, info, owner, generator, factory, collector, fee), // should be contract restricted

        // user functions
        ExecuteMsg::Merge { token, amount } => merge(deps, env, info, token, amount),
        ExecuteMsg::Split { token, amount } => split(deps, env, info, token, amount),
        ExecuteMsg::Unstake { token, amount } => unstake(deps, env, info, token, amount),
        ExecuteMsg::UpdateStakingMode { token, mode } => update_staking_mode(deps, env, info, token, mode),
        ExecuteMsg::ClaimRewards { } => claim_rewards(deps, env, info),

        // internal functions
        ExecuteMsg::Mint { user, token, amount } => mint(deps, env, info, user, token, amount),
        ExecuteMsg::Burn { user, token, amount } => burn(deps, env, info, user, token, amount),
        ExecuteMsg::CreateTokens { token } => create_tokens(deps, env, info, token),
        ExecuteMsg::UpdateLPRewards { token } => update_lp_rewards(deps, env, info, token),
        ExecuteMsg::SendStakerRewards { staker } => send_staker_rewards(deps, env, info, staker),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let cw20_sender: Addr = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => bond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
        Ok(Cw20HookMsg::Unbond {}) => unbond(deps, env, info.sender, cw20_sender, cw20_msg.amount),
        Ok(Cw20HookMsg::Stake { amount }) => stake(deps, env, info.sender, cw20_sender, cw20_msg.amount),
        Err(_) => Err(StdError::generic_err(format!("Invalid CW20 Message"))),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

// only executable by owner
pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    generator: Option<String>,
    factory: Option<String>,
    collector: Option<String>,
    fee: Option<Decimal>,
) -> StdResult<Response> {
    let conf = CONFIG.load(deps.storage)?;

    if info.sender.as_str() != conf.owner {
        return Err(StdError::generic_err(format!("Unauthorized")));
    }

    if let Some(creator) = owner {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.owner = creator;
            Ok(last_config)
        })?;
    }

    if let Some(generator_contract) = generator {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.generator = generator_contract;
            Ok(last_config)
        })?;
    }

    if let Some(factory_contract) = factory {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.factory = factory_contract;
            Ok(last_config)
        })?;
    }

    if let Some(fee_contract) = collector {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.collector = fee_contract;
            Ok(last_config)
        })?;
    }

    if let Some(prism_fee) = fee {
        CONFIG.update(deps.storage, |mut last_config| -> StdResult<Config> {
            last_config.fee = prism_fee;
            Ok(last_config)
        })?;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}