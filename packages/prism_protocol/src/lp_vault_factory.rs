use cosmwasm_std::{Addr, Decimal};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub gov: String,
    pub prism_yasset_pair: String,
    pub collector: String,
    pub fee: Decimal,
    pub token_code_id: u64,
    pub yasset_contract_id: u64,
    pub yasset_x_contract_id: u64,
    pub reward_dist_contract_id: u64,

    // astroport
    pub lp_astro_vault_id: u64,
    pub generator: String,
    pub factory: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // update config
    UpdateConfig {
        owner: Option<Addr>,
        gov: Option<Addr>,
        prism_yasset_pair: Option<Addr>,
        collector: Option<Addr>,
        yasset_contract_id: Option<u64>,
        yasset_x_contract_id: Option<u64>,
        reward_dist_contract_id: Option<u64>,
        fee: Option<Decimal>,
        token_code_id: Option<u64>,
    },

    // support new LP
    CreateNewVault {
        amm: u64,
        lp: Addr,
    },

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Vault { amm: u64, lp: Addr, },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // owner of contract
    pub owner: Addr,

    // address of governance contract
    pub gov: Addr,

    // needed for yasset-x-staking
    pub prism_yasset_pair: Addr,

    // address of collector contract
    pub collector: Addr,

    // prism LP fee of 15%
    pub fee: Decimal,

    // for token instantiation
    pub token_code_id: u64,

    // for yasset-staking instantiation
    pub yasset_contract_id: u64,

    // for yasset-x-staking instantiation
    pub yasset_x_contract_id: u64,

    // for reward-distribution instantiation
    pub reward_dist_contract_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LPContracts {
    pub amm: u64,
    pub lp: Addr,
    pub clp_contract: Addr,
    pub plp_contract: Addr,
    pub ylp_contract: Addr,
    pub collector: Addr,
    pub yasset_contract: Addr,
    pub yasset_x_contract: Addr,
    pub reward_dist_contract: Addr,
    pub vault: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AstroConfig {
    pub lp_astro_vault_id: u64,
    pub generator: Addr,
    pub factory: Addr,
}