use cosmwasm_std::{
    to_binary, Addr, Deps, Env, QuerierWrapper, QueryRequest, StdError, StdResult, Uint128,
    WasmQuery,
};

use astroport::asset::{Asset, AssetInfo, PairInfo};
use astroport::factory::{
    ConfigResponse as AstroFactoryConfigResponse, QueryMsg as AstroFactoryQueryMsg,
};
use astroport::generator::{
    PendingTokenResponse, QueryMsg as AstroGeneratorQueryMsg, RewardInfoResponse,
};
use astroport::pair::{PoolResponse, QueryMsg as AstroPairQueryMsg};
use cw20::{Cw20QueryMsg, TokenInfoResponse};
use prism_protocol::lp_vault::{Config, ConfigResponse};

use crate::state::{CONFIG, LP_IDS, LP_INFOS};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    config.as_res()
}

pub fn query_token_info(
    querier: &QuerierWrapper,
    contract_addr: Addr,
) -> StdResult<TokenInfoResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.into_string(),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))
}

pub fn query_all_pairs(deps: Deps, querier: &QuerierWrapper) -> StdResult<Vec<PairInfo>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // grab all pairs info from astroport factory
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory,
        msg: to_binary(&AstroFactoryQueryMsg::Pairs {
            limit: None,
            start_after: None,
        })?,
    }))
}

// called whenever a new LP token is found
// IDEA: we can keep a cache of astroport pairs to avoid the QueryMsg call for future tokens
pub fn query_pair_info(
    deps: Deps,
    querier: &QuerierWrapper,
    token_addr: Addr,
) -> StdResult<PairInfo> {
    // find PairInfo with equivalent LP token
    query_all_pairs(deps, querier)?
        .into_iter()
        .find(|x| x.liquidity_token == token_addr)
        .ok_or_else(|| StdError::generic_err("LP Token not found in Astroport"))
}

pub fn query_pool_info(
    deps: Deps,
    querier: &QuerierWrapper,
    token_addr: Addr,
) -> StdResult<PoolResponse> {
    let lp_id = LP_IDS.load(deps.storage, &token_addr)?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.into_string(),
        msg: to_binary(&AstroPairQueryMsg::Pool {})?,
    }))
}

pub fn query_factory_config(
    deps: Deps,
    querier: &QuerierWrapper,
) -> StdResult<AstroFactoryConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory,
        msg: to_binary(&AstroFactoryQueryMsg::Config {})?,
    }))
}

pub fn query_generator_rewards(
    deps: Deps,
    querier: &QuerierWrapper,
    token: Addr,
) -> StdResult<Vec<AssetInfo>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // query for generator reward infos
    let gen_reward_info: RewardInfoResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator,
            msg: to_binary(&AstroGeneratorQueryMsg::RewardInfo { lp_token: token })?,
        }))?;

    // if there exists a proxy reward, send back both
    // QUES: is there some cleaner way to do this?
    match gen_reward_info.proxy_reward_token {
        Some(addr) => Ok(vec![
            AssetInfo::Token {
                contract_addr: gen_reward_info.base_reward_token.clone(),
            },
            AssetInfo::Token {
                contract_addr: addr,
            },
        ]),
        None => Ok(vec![AssetInfo::Token {
            contract_addr: gen_reward_info.base_reward_token.clone(),
        }]),
    }
}

pub fn query_pending_generator_rewards(
    deps: Deps,
    env: Env,
    querier: &QuerierWrapper,
    token: Addr,
) -> StdResult<PendingTokenResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory,
        msg: to_binary(&AstroGeneratorQueryMsg::PendingToken {
            lp_token: token,
            user: env.contract.address,
        })?,
    }))
}

pub fn query_lp_burn_rewards(
    deps: Deps,
    querier: &QuerierWrapper,
    token: Addr,
    amount: Uint128,
) -> StdResult<Vec<Asset>> {
    let lp_id = LP_IDS.load(deps.storage, &token)?;
    let lp_info = LP_INFOS.load(deps.storage, lp_id.into())?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.into_string(),
        msg: to_binary(&AstroPairQueryMsg::Share { amount })?,
    }))
}
