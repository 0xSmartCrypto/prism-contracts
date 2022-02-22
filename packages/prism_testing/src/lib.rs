//! This crate provides utilities to write unit tests.
//!
//! There's shouldn't be any production code in this crate, since it won't
//! compile down to Wasm (because it imports CosmWasm libraries with
//! "#[cfg(not(target_arch = "wasm32"))]" attributes, for example:
//! https://github.com/CosmWasm/cosmwasm/blob/v0.16.4/packages/std/src/lib.rs#L92-L96)

pub mod mock_querier;
