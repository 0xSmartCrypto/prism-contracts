use cosmwasm_std::{
    to_binary, Addr, Deps, Env, QuerierWrapper, QueryRequest, StdError, StdResult, Uint128,
    WasmQuery,
};

use astroport::asset::{Asset, AssetInfo, PairInfo};
use astroport::factory::QueryMsg as AstroFactoryQueryMsg;
use astroport::generator::{
    PendingTokenResponse, QueryMsg as AstroGeneratorQueryMsg, RewardInfoResponse,
};
use astroport::pair::{PoolResponse, QueryMsg as AstroPairQueryMsg};
use prism_protocol::terraswap_lp_vault::{Config, ConfigResponse, LPInfo};

use crate::state::{CONFIG, LP_INFO};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    config.as_res()
}

pub fn query_all_pairs(deps: Deps, querier: &QuerierWrapper) -> StdResult<Vec<PairInfo>> {
    let config: Config = CONFIG.load(deps.storage)?;

    // grab all pairs info from astroport factory
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.into_string(),
        msg: to_binary(&AstroFactoryQueryMsg::Pairs {
            limit: None,
            start_after: None,
        })?,
    }))
}

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

pub fn query_pool_info(deps: Deps, querier: &QuerierWrapper) -> StdResult<PoolResponse> {
    let lp_info = LP_INFO.load(deps.storage)?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.into_string(),
        msg: to_binary(&AstroPairQueryMsg::Pool {})?,
    }))
}

pub fn query_generator_rewards_info(
    deps: Deps,
    querier: &QuerierWrapper,
) -> StdResult<Vec<AssetInfo>> {
    let config = CONFIG.load(deps.storage)?;
    let lp_info = LP_INFO.load(deps.storage)?;

    // query for generator reward infos
    let gen_reward_info: RewardInfoResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator.into_string(),
            msg: to_binary(&AstroGeneratorQueryMsg::RewardInfo {
                lp_token: lp_info.lp_contract,
            })?,
        }))?;

    // if there exists a proxy reward, send back both
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

#[allow(clippy::or_fun_call)]
pub fn query_pending_generator_rewards(
    deps: Deps,
    env: Env,
    querier: &QuerierWrapper,
) -> StdResult<Vec<Asset>> {
    let config = CONFIG.load(deps.storage)?;
    let lp_info = LP_INFO.load(deps.storage)?;

    let res: PendingTokenResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.into_string(),
        msg: to_binary(&AstroGeneratorQueryMsg::PendingToken {
            lp_token: lp_info.lp_contract,
            user: env.contract.address,
        })?,
    }))?;
    let pending_proxy = res.pending_on_proxy.unwrap_or(Uint128::zero());

    // form into Asset types for easy transfer
    let mut assets = vec![Asset {
        info: lp_info.generator_reward_info[0].clone(),
        amount: res.pending,
    }];
    if pending_proxy > Uint128::zero() {
        assets.push(Asset {
            info: lp_info.generator_reward_info[1].clone(),
            amount: pending_proxy,
        });
    }

    Ok(assets)
}

pub fn query_lp_burn_rewards(
    deps: Deps,
    querier: &QuerierWrapper,
    amount: Uint128,
) -> StdResult<Vec<Asset>> {
    let lp_info = LP_INFO.load(deps.storage)?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.into_string(),
        msg: to_binary(&AstroPairQueryMsg::Share { amount })?,
    }))
}

pub fn query_lp_info(deps: Deps) -> StdResult<LPInfo> {
    LP_INFO.load(deps.storage)
}

pub fn query_bonded_amount(deps: Deps) -> StdResult<Uint128> {
    // should this be cLP?
    let lp_info = LP_INFO.load(deps.storage)?;
    Ok(lp_info.amt_lp)
}
