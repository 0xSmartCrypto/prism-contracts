use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use cosmwasm_std::{Addr, Uint128};
use prism_protocol::basset_vault::{ConfigResponse, StateResponse};

use crate::error::{ContractError, ContractResult};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub asset_name: String,
    pub asset_contract: Addr,
    pub asset_reward_contract: Addr,
    pub asset_reward_denom: String,
    pub casset_contract: Addr,
    pub yasset_contract: Addr,
    pub passet_contract: Addr,
    pub reward_distribution_contract: Addr,
    pub initialized: bool,
    pub token_admin: Addr,
    pub token_code_id: u64,
}

impl Config {
    pub fn as_res(&self) -> ConfigResponse {
        ConfigResponse {
            owner: self.owner.to_string(),
            asset_name: self.asset_name.clone(),
            asset_contract: self.asset_contract.to_string(),
            asset_reward_contract: self.asset_reward_contract.to_string(),
            asset_reward_denom: self.asset_reward_denom.clone(),
            casset_contract: self.casset_contract.to_string(),
            yasset_contract: self.yasset_contract.to_string(),
            passet_contract: self.passet_contract.to_string(),
            reward_distribution_contract: self.reward_distribution_contract.to_string(),
            initialized: self.initialized,
            token_admin: self.token_admin.to_string(),
            token_code_id: self.token_code_id,
        }
    }

    pub fn assert_initialized(self) -> ContractResult<Config> {
        if self.initialized {
            Ok(self)
        } else {
            Err(ContractError::NotInitialized {})
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub total_bond_amount: Uint128,
    pub last_index_modification: u64,
}

impl State {
    pub fn as_res(&self) -> StateResponse {
        StateResponse {
            total_bond_amount: self.total_bond_amount,
            last_index_modification: self.last_index_modification,
        }
    }
}
