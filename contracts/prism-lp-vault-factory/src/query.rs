use cosmwasm_std::{to_binary, Addr, Deps, QuerierWrapper, QueryRequest, StdResult, WasmQuery};

use cw20::{Cw20QueryMsg, TokenInfoResponse};

use prism_protocol::collector::{
    ConfigResponse as CollectorConfigResponse, QueryMsg as CollectorQueryMsg,
};
use prism_protocol::lp_vault_factory::{AstroConfig, Config, LPContracts, TerraswapConfig};

// would ideally like to consolidate the astroport/terraswap stuff..
use astroport::asset::{AssetInfo as AstroAssetInfo, PairInfo as AstroPairInfo};
use astroport::factory::{
    PairsResponse as AstroPairInfoResponse, QueryMsg as AstroFactoryQueryMsg,
};
use astroport::generator::{QueryMsg as AstroGeneratorQueryMsg, RewardInfoResponse};

use terraswap::asset::PairInfo as TerraPairInfo;
use terraswap::factory::{
    PairsResponse as TerraPairInfoResponse, QueryMsg as TerraswapFactoryQueryMsg,
};

use crate::error::{ContractError, ContractResult};
use crate::state::{ASTRO_CONFIG, CONFIG, TERRASWAP_CONFIG, VAULTS};

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn query_vault(deps: Deps, lp: &Addr) -> StdResult<LPContracts> {
    VAULTS.load(deps.storage, lp)
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

pub fn query_collector_config(
    querier: &QuerierWrapper,
    collector: Addr,
) -> StdResult<CollectorConfigResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: collector.into_string(),
        msg: to_binary(&CollectorQueryMsg::Config {})?,
    }))
}

pub fn query_generator_rewards(
    deps: Deps,
    querier: &QuerierWrapper,
    token: Addr,
) -> StdResult<Vec<AstroAssetInfo>> {
    let config: AstroConfig = ASTRO_CONFIG.load(deps.storage)?;

    // query for generator reward infos
    let gen_reward_info: RewardInfoResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator.into_string(),
            msg: to_binary(&AstroGeneratorQueryMsg::RewardInfo { lp_token: token })?,
        }))?;

    // if there exists a proxy reward, send back both
    match gen_reward_info.proxy_reward_token {
        Some(addr) => Ok(vec![
            AstroAssetInfo::Token {
                contract_addr: gen_reward_info.base_reward_token.clone(),
            },
            AstroAssetInfo::Token {
                contract_addr: addr,
            },
        ]),
        None => Ok(vec![AstroAssetInfo::Token {
            contract_addr: gen_reward_info.base_reward_token.clone(),
        }]),
    }
}

pub fn query_all_astroport_pairs(
    deps: Deps,
    querier: &QuerierWrapper,
) -> StdResult<Vec<AstroPairInfo>> {
    let config = ASTRO_CONFIG.load(deps.storage)?;

    // grab all pairs info from astroport factory
    let res: AstroPairInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.into_string(),
        msg: to_binary(&AstroFactoryQueryMsg::Pairs {
            limit: None,
            start_after: None,
        })?,
    }))?;
    Ok(res.pairs)
}

pub fn query_astroport_pair_info(
    deps: Deps,
    querier: &QuerierWrapper,
    token_addr: Addr,
) -> ContractResult<AstroPairInfo> {
    // find PairInfo with equivalent LP token
    query_all_astroport_pairs(deps, querier)?
        .into_iter()
        .find(|x| x.liquidity_token == token_addr)
        .ok_or(ContractError::DoesNotExist {})
}

pub fn query_all_terraswap_pairs(
    deps: Deps,
    querier: &QuerierWrapper,
) -> StdResult<Vec<TerraPairInfo>> {
    let config = TERRASWAP_CONFIG.load(deps.storage)?;

    // grab all pairs info from astroport factory
    let res: TerraPairInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.factory.into_string(),
        msg: to_binary(&TerraswapFactoryQueryMsg::Pairs {
            limit: None,
            start_after: None,
        })?,
    }))?;
    Ok(res.pairs)
}

pub fn query_terraswap_pair_info(
    deps: Deps,
    querier: &QuerierWrapper,
    token_addr: Addr,
) -> ContractResult<TerraPairInfo> {
    // find PairInfo with equivalent LP token
    query_all_terraswap_pairs(deps, querier)?
        .into_iter()
        .find(|x| x.liquidity_token == token_addr)
        .ok_or(ContractError::DoesNotExist {})
}

pub fn query_astro_amm_info(deps: Deps) -> StdResult<AstroConfig> {
    ASTRO_CONFIG.load(deps.storage)
}

pub fn query_terraswap_amm_info(deps: Deps) -> StdResult<TerraswapConfig> {
    TERRASWAP_CONFIG.load(deps.storage)
}
