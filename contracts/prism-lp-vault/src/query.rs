use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env, Decimal, StdResult, Uint128, StdError, CosmosMsg, WasmMsg, QueryRequest, QuerierWrapper, WasmQuery, Response, to_binary,};

use crate::state::{CONFIG, LP_IDS, LP_INFOS};

use prism_protocol::lp_vault::{Config, ConfigResponse};
use astroport::asset::{PairInfo, AssetInfo, Asset};
use astroport::pair::{QueryMsg as AstroPairQueryMsg, PoolResponse};
use astroport::factory::{QueryMsg as AstroFactoryQueryMsg, ConfigResponse as AstroFactoryConfigResponse};
use astroport::generator::{QueryMsg as AstroGeneratorQueryMsg, RewardInfoResponse, PendingTokenResponse};

use cw20::{Cw20QueryMsg, TokenInfoResponse};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    Ok(config.as_res()?)
}

// use querier.query_wasm_smart instead?

pub fn query_token_info(querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<TokenInfoResponse> {
    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(contract_addr),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?)
}

pub fn query_all_pairs(deps: Deps, querier: &QuerierWrapper) -> StdResult<Vec<PairInfo>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // grab all pairs from astroport
    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.clone(),
        msg: to_binary(&AstroFactoryQueryMsg::Pairs { 
            limit: None,
            start_after: None,
        })?,
    }))?)
}

// pretty inefficient for now... may be some other way to go from liquidity token contract addr -> pair contract addr
pub fn query_pair_info(deps: Deps, querier: &QuerierWrapper, contract_addr: Addr) -> StdResult<PairInfo> {
    let config: Config = CONFIG.load(deps.storage)?;
    
    // find PairInfo with equivalent LP token
    Ok(query_all_pairs(deps, &querier)?
        .into_iter()
        .find(|x| x.liquidity_token == contract_addr)
        .ok_or_else(|| {
            StdError::generic_err("LP Token not found in Astroport")
        })?
    )
}


pub fn query_pool_info(deps: Deps, querier: &QuerierWrapper, token_addr: Addr) -> StdResult<PoolResponse> {
    let lp_id = LP_IDS.load(deps.storage, &token_addr)?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.clone().into_string(),
        msg: to_binary(&AstroPairQueryMsg::Pool { })?,
    }))?)
}

pub fn query_factory_config(deps: Deps, querier: &QuerierWrapper) -> StdResult<AstroFactoryConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    // query for factory config and return its token code id
    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.clone(),
        msg: to_binary(&AstroFactoryQueryMsg::Config {})?,
    }))?)
}

pub fn query_generator_rewards(deps: Deps, querier: &QuerierWrapper, token: Addr) -> StdResult<Vec<AssetInfo>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // query for generator reward infos
    let gen_reward_info: RewardInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.generator.clone(),
        msg: to_binary(&AstroGeneratorQueryMsg::RewardInfo {
            lp_token: token,
        })?,
    }))?;
    
    // if there exists a proxy reward, send back both
    // looks kinda ugly
    match gen_reward_info.proxy_reward_token {
        Some(addr) => {
            Ok(vec![
                AssetInfo::Token { contract_addr: gen_reward_info.base_reward_token.clone() },
                AssetInfo::Token { contract_addr: addr.clone() },
            ])
        },
        None => {
            Ok(vec![
                AssetInfo::Token { contract_addr: gen_reward_info.base_reward_token.clone() },
            ])
        }
    }
}

pub fn query_pending_generator_rewards(deps: Deps, env: Env, querier: &QuerierWrapper, token: Addr) -> StdResult<PendingTokenResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.clone(),
        msg: to_binary(&AstroGeneratorQueryMsg::PendingToken { 
            lp_token: token.clone(),
            user: env.contract.address.clone(),
        })?,
    }))?)
}

pub fn query_lp_burn_rewards(deps: Deps, querier: &QuerierWrapper, token: Addr, amount: Uint128) -> StdResult<Vec<Asset>> {
    let lp_id = LP_IDS.load(deps.storage, &token)?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    Ok(querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.clone().into_string(),
        msg: to_binary(&AstroPairQueryMsg::Share { 
            amount,
        })?,
    }))?)
}