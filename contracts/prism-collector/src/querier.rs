use crate::state::Config;
use astroport::asset::AssetInfo as AstroAssetInfo;
use cosmwasm_std::{Addr, Binary, DepsMut, QueryRequest, StdResult, WasmQuery};
use cw_asset::AssetInfo;
use cw_storage_plus::Path;
use prismswap::asset::PrismSwapAssetInfo;
use prismswap::querier::query_pair_info;
use serde::{Deserialize, Serialize};

// PairConfig copied directly from prismswap factory state.rs
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PairConfig {
    pub pair_address: Addr,
    pub fee_config: prismswap::factory::FeeConfig,
}

// pair_key copied directly from prismswap factory state.rs
pub fn pair_key(asset_infos: &[AssetInfo; 2]) -> Vec<u8> {
    let mut asset_infos = asset_infos.to_vec();
    asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat()
}

// astro_pair_key copied directly from astroport factory state.rs
pub fn astro_pair_key(asset_infos: &[AstroAssetInfo; 2]) -> Vec<u8> {
    let mut asset_infos = asset_infos.to_vec();
    asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat()
}

pub fn query_prismswap_pair(
    deps: &DepsMut,
    config: &Config,
    asset_infos: &[AssetInfo; 2],
) -> Option<Addr> {
    query_pair_info(&deps.querier, &config.prismswap_factory, asset_infos)
        .ok()
        .map(|x| x.contract_addr)
}

pub fn query_astroport_pair(
    deps: &DepsMut,
    config: &Config,
    asset_infos: &[AssetInfo; 2],
) -> Option<Addr> {
    let astro_asset_infos: [astroport::asset::AssetInfo; 2] =
        [asset_infos[0].clone().into(), asset_infos[1].clone().into()];

    astroport::querier::query_pair_info(
        &deps.querier,
        config.astroport_factory.clone(),
        &astro_asset_infos,
    )
    .ok()
    .map(|x| x.contract_addr)
}

pub fn query_prismswap_pair_raw(
    deps: &DepsMut,
    config: &Config,
    asset_infos: &[AssetInfo; 2],
) -> Option<Addr> {
    let path: Path<PairConfig> = Path::new("pair_config".as_bytes(), &[&pair_key(asset_infos)]);
    let pair_config: StdResult<PairConfig> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: config.prismswap_factory.to_string(),
            key: Binary::from(&*path),
        }));
    pair_config.ok().map(|x| x.pair_address)
}

pub fn query_astroport_pair_raw(
    deps: &DepsMut,
    config: &Config,
    asset_infos: &[AssetInfo; 2],
) -> Option<Addr> {
    let astro_asset_infos: [astroport::asset::AssetInfo; 2] =
        [asset_infos[0].clone().into(), asset_infos[1].clone().into()];

    let path: Path<Addr> = Path::new(
        "pair_info".as_bytes(),
        &[&astro_pair_key(&astro_asset_infos)],
    );
    let pair_addr: StdResult<Addr> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: config.astroport_factory.to_string(),
        key: Binary::from(&*path),
    }));
    pair_addr.ok()
}
