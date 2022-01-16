use cosmwasm_std::{to_binary, Addr, Deps, QuerierWrapper, QueryRequest, StdResult, WasmQuery};

use cw20::{Cw20QueryMsg, TokenInfoResponse};

use prism_protocol::collector::{
    ConfigResponse as CollectorConfigResponse, QueryMsg as CollectorQueryMsg,
};
use prism_protocol::lp_vault_factory::{AstroConfig, Config, LPContracts};

use astroport::asset::{AssetInfo, PairInfo};
use astroport::factory::QueryMsg as AstroFactoryQueryMsg;
use astroport::generator::{QueryMsg as AstroGeneratorQueryMsg, RewardInfoResponse};

use crate::error::{ContractError, ContractResult};
use crate::state::{ASTRO_CONFIG, CONFIG, VAULTS};

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn query_vault(deps: Deps, amm: u64, lp: &Addr) -> StdResult<LPContracts> {
    VAULTS.load(deps.storage, (amm.into(), lp))
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
) -> StdResult<Vec<AssetInfo>> {
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

pub fn query_all_pairs(deps: Deps, querier: &QuerierWrapper) -> StdResult<Vec<PairInfo>> {
    let config: AstroConfig = ASTRO_CONFIG.load(deps.storage)?;

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
) -> ContractResult<PairInfo> {
    // find PairInfo with equivalent LP token
    query_all_pairs(deps, querier)?
        .into_iter()
        .find(|x| x.liquidity_token == token_addr)
        .ok_or(ContractError::DoesNotExist {})
}
