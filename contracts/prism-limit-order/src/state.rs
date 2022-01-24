use cw_storage_plus::{Bound, Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Order, StdResult, Storage, Uint128};
use prismswap::asset::{Asset, AssetInfo};

use prism_protocol::limit_order::{ConfigResponse, OrderBy, OrderResponse};

pub const CONFIG: Item<Config> = Item::new("config");
pub const LAST_ORDER_ID: Item<u64> = Item::new("last_order_id");
pub const ORDERS: Map<&[u8], OrderInfo> = Map::new("orders");
pub const ORDERS_BY_USER: Map<(&[u8], &[u8]), bool> = Map::new("orders_by_user");
pub const PAIRS: Map<&[u8], Addr> = Map::new("pairs");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub base_denom: String,
    pub owner: Addr,
    pub fee_collector_addr: Addr,
    pub prism_token: Addr,
    pub prism_ust_pair: Addr,
    pub order_fee: Decimal,
    pub min_fee_value: Uint128,
    pub executor_fee_portion: Decimal,
    pub excess_collactor_addr: Addr,
}

impl Config {
    pub fn as_res(&self) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            base_denom: self.base_denom.clone(),
            owner: self.owner.to_string(),
            fee_collector_addr: self.fee_collector_addr.to_string(),
            prism_token: self.prism_token.to_string(),
            prism_ust_pair: self.prism_ust_pair.to_string(),
            order_fee: self.order_fee,
            min_fee_value: self.min_fee_value,
            executor_fee_portion: self.executor_fee_portion,
            excess_collector_addr: self.excess_collactor_addr.to_string(),
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OrderInfo {
    pub order_id: u64,
    pub bidder_addr: Addr,
    pub pair_addr: Addr,
    pub inter_pair_addr: Option<Addr>,
    pub offer_asset: Asset,
    pub ask_asset: Asset,
}

impl OrderInfo {
    pub fn as_res(&self) -> StdResult<OrderResponse> {
        let res = OrderResponse {
            order_id: self.order_id,
            bidder_addr: self.bidder_addr.to_string(),
            pair_addr: self.pair_addr.to_string(),
            inter_pair_addr: self.inter_pair_addr.clone().map(|pair| pair.to_string()),
            offer_asset: self.offer_asset.clone(),
            ask_asset: self.ask_asset.clone(),
        };
        Ok(res)
    }
}

pub fn store_new_order(storage: &mut dyn Storage, order: &mut OrderInfo) -> StdResult<()> {
    let new_id: u64 = LAST_ORDER_ID.load(storage)? + 1u64;
    order.order_id = new_id;

    ORDERS.save(storage, &new_id.to_be_bytes(), order)?;
    ORDERS_BY_USER.save(
        storage,
        (order.bidder_addr.as_bytes(), &new_id.to_be_bytes()),
        &true,
    )?;
    LAST_ORDER_ID.save(storage, &new_id)?;

    Ok(())
}

pub fn remove_order(storage: &mut dyn Storage, order: &OrderInfo) {
    ORDERS.remove(storage, &order.order_id.to_be_bytes());
    ORDERS_BY_USER.remove(
        storage,
        (order.bidder_addr.as_bytes(), &order.order_id.to_be_bytes()),
    );
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn read_orders_by_user(
    storage: &dyn Storage,
    user: &Addr,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<OrderInfo>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (
            calc_range_start(start_after).map(Bound::exclusive),
            None,
            Order::Ascending,
        ),
        _ => (
            None,
            calc_range_end(start_after).map(Bound::exclusive),
            Order::Descending,
        ),
    };

    ORDERS_BY_USER
        .prefix(user.as_bytes())
        .range(storage, start, end, order_by)
        .take(limit)
        .map(|item| {
            let (k, _) = item?;
            ORDERS.load(storage, &k)
        })
        .collect()
}

pub fn read_orders(
    storage: &dyn Storage,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<OrderInfo>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (
            calc_range_start(start_after).map(Bound::exclusive),
            None,
            Order::Ascending,
        ),
        _ => (
            None,
            calc_range_end(start_after).map(Bound::exclusive),
            Order::Descending,
        ),
    };

    ORDERS
        .range(storage, start, end, order_by)
        .take(limit)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect()
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| {
        let mut v = id.to_be_bytes().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| id.to_be_bytes().to_vec())
}

pub fn generate_pair_key(asset_infos: &[AssetInfo; 2]) -> Vec<u8> {
    let mut asset_infos = asset_infos.to_vec();
    asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat()
}

pub fn is_existing_pair(storage: &dyn Storage, asset_infos: &[AssetInfo; 2]) -> bool {
    let key = generate_pair_key(asset_infos);
    PAIRS.may_load(storage, &key).unwrap().is_some()
}
