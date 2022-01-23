use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmQuery,
};

use prism_protocol::terraswap_lp_vault::{Config, ConfigResponse, LPInfo};
use terraswap::asset::{Asset, PairInfo};
use terraswap::factory::{
    PairsResponse as TerraPairInfoResponse, QueryMsg as TerraswapFactoryQueryMsg,
};
use terraswap::pair::{PoolResponse, QueryMsg as TerraPairQueryMsg};

use crate::error::{ContractError, ContractResult};
use crate::state::{CONFIG, LP_INFO};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    config.as_res()
}

pub fn query_all_pairs(deps: Deps, querier: &QuerierWrapper) -> StdResult<Vec<PairInfo>> {
    let config = CONFIG.load(deps.storage)?;
    // grab all pairs info from terraswap factory
    let res: TerraPairInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.into_string(),
        msg: to_binary(&TerraswapFactoryQueryMsg::Pairs {
            limit: None,
            start_after: None,
        })?,
    }))?;
    Ok(res.pairs)
}

pub fn query_pair_info(
    deps: Deps,
    querier: &QuerierWrapper,
    token_addr: Addr,
) -> ContractResult<PairInfo> {
    // find PairInfo with equivalent LP token
    query_all_pairs(deps, querier)?
        .into_iter()
        .find(|x| x.liquidity_token == token_addr)
        .ok_or(ContractError::DoesNotExist {})
}

pub fn query_pool_info(deps: Deps, querier: &QuerierWrapper) -> StdResult<PoolResponse> {
    let lp_info = LP_INFO.load(deps.storage)?;

    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_info.pair_contract.into_string(),
        msg: to_binary(&TerraPairQueryMsg::Pool {})?,
    }))
}

pub fn query_lp_burn_rewards(
    deps: Deps,
    querier: &QuerierWrapper,
    amount: Uint128,
) -> StdResult<Vec<Asset>> {
    let res = query_pool_info(deps, querier)?;
    let mut share_ratio = Decimal::zero();
    if !res.total_share.is_zero() {
        share_ratio = Decimal::from_ratio(amount, res.total_share);
    }

    Ok(res
        .assets
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: a.amount * share_ratio,
        })
        .collect())
}

pub fn query_lp_info(deps: Deps) -> StdResult<LPInfo> {
    LP_INFO.load(deps.storage)
}

pub fn query_bonded_amount(deps: Deps) -> StdResult<Uint128> {
    let lp_info = LP_INFO.load(deps.storage)?;
    Ok(lp_info.amt_clp)
}
