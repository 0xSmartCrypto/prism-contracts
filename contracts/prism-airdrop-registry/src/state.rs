use prism_common::de::deserialize_key;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Order, StdResult, Storage};
use cw_storage_plus::{Bound, Item, Map};
use prism_protocol::airdrop_registry::{AirdropInfo, AirdropInfoElem};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub vault_contract: Addr,
    pub airdrop_tokens: Vec<Addr>,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// airdrop token -> AirdropInfo
pub const AIRDROP_INFO: Map<&Addr, AirdropInfo> = Map::new("airdrop_info");

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    CONFIG.save(storage, config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    CONFIG.load(storage)
}

pub fn store_airdrop_info(
    storage: &mut dyn Storage,
    airdrop_token: &Addr,
    airdrop_info: &AirdropInfo,
) -> StdResult<()> {
    AIRDROP_INFO.save(storage, airdrop_token, airdrop_info)
}

pub fn update_airdrop_info(
    storage: &mut dyn Storage,
    airdrop_token: &Addr,
    airdrop_info: &AirdropInfo,
) -> StdResult<()> {
    AIRDROP_INFO.update(storage, airdrop_token, |_| -> StdResult<_> {
        Ok(airdrop_info.clone())
    })?;
    Ok(())
}

pub fn remove_airdrop_info(storage: &mut dyn Storage, airdrop_token: &Addr) -> StdResult<()> {
    AIRDROP_INFO.remove(storage, airdrop_token);
    Ok(())
}

pub fn read_airdrop_info(storage: &dyn Storage, airdrop_token: &Addr) -> StdResult<AirdropInfo> {
    AIRDROP_INFO.load(storage, airdrop_token)
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_all_airdrop_infos(
    storage: &dyn Storage,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<Vec<AirdropInfoElem>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|addr| Bound::exclusive(addr.as_bytes()));

    let infos: Vec<AirdropInfoElem> = AIRDROP_INFO
        .range(storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (k, v) = item.unwrap();
            let airdrop_token = deserialize_key::<Addr>(k).unwrap();
            AirdropInfoElem {
                airdrop_token: airdrop_token.to_string(),
                info: v,
            }
        })
        .collect();

    Ok(infos)
}
