#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Addr, CosmosMsg, Decimal, Deps, DepsMut, Env, Reply, ReplyOn, Response,
    SubMsg, WasmMsg,
};

use prism_common::parse_reply_instantiate_data;
use prism_protocol::astroport_lp_vault::{
    ExecuteMsg as AstroVaultExecuteMsg, InstantiateMsg as AstroVaultInstantiateMsg,
};
use prism_protocol::lp_vault_factory::LPContracts;
use prism_protocol::reward_distribution::InstantiateMsg as RewardDistInstantiateMsg;
use prism_protocol::terraswap_lp_vault::{
    ExecuteMsg as TerraswapVaultExecuteMsg, InstantiateMsg as TerraswapVaultInstantiateMsg,
};
use prism_protocol::yasset_staking::InstantiateMsg as YassetInstantiateMsg;
use prism_protocol::yasset_staking_x::InstantiateMsg as YassetXInstantiateMsg;

use astroport::asset::AssetInfo as AstroAssetInfo;
use terraswap::asset::AssetInfo as TerraAssetInfo;

use crate::error::{ContractError, ContractResult};
use crate::query::{
    query_astroport_pair_info, query_collector_config, query_generator_rewards,
    query_terraswap_pair_info, query_token_info,
};
use crate::state::{ASTRO_CONFIG, CONFIG, TEMP_LP_INFO, TERRASWAP_CONFIG, VAULTS};

use cw20::MinterResponse;
use cw20_base::msg::InstantiateMsg as TokenInstantiateMsg;

use terra_cosmwasm::TerraMsgWrapper;

const CLP_INSTANTIATE_REPLY_ID: u64 = 1;
const PLP_INSTANTIATE_REPLY_ID: u64 = 2;
const YLP_INSTANTIATE_REPLY_ID: u64 = 3;
const YASSET_STAKING_REPLY_ID: u64 = 4;
const YASSET_X_STAKING_REPLY_ID: u64 = 5;
const LP_VAULT_REPLY_ID: u64 = 6;
const REWARD_DIST_INSTANTIATE_REPLY_ID: u64 = 7;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn create_new_lp_vault(
    deps: DepsMut,
    env: Env,
    lp: Addr,
) -> ContractResult<Response<TerraMsgWrapper>> {
    // create c/p/yLP tokens
    let cfg = CONFIG.load(deps.storage)?;
    let token_info = query_token_info(&deps.querier, lp)?;
    let submessages = vec![
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: cfg.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "c".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "c".to_string())?,
                    decimals: token_info.decimals,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                    marketing: None,
                })?,
                funds: vec![],
                admin: None,
                label: String::from("Prism cLP token"),
            }
            .into(),
            id: CLP_INSTANTIATE_REPLY_ID,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: cfg.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "p".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "p".to_string())?,
                    decimals: token_info.decimals,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                    marketing: None,
                })?,
                funds: vec![],
                admin: None,
                label: String::from("Prism pLP token"),
            }
            .into(),
            id: PLP_INSTANTIATE_REPLY_ID,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: cfg.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "y".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "y".to_string())?,
                    decimals: token_info.decimals,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                    marketing: None,
                })?,
                funds: vec![],
                admin: None,
                label: String::from("Prism yLP token"),
            }
            .into(),
            id: YLP_INSTANTIATE_REPLY_ID,
            gas_limit: None,
            reply_on: ReplyOn::Success,
        },
    ];

    Ok(Response::new().add_submessages(submessages))
}

#[cfg_attr(not(feature = "library"), entry_point)]
#[allow(dead_code)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> ContractResult<Response> {
    let id = msg.id;
    let res = parse_reply_instantiate_data(msg).map_err(|_| ContractError::ParseError {})?;

    // get contract addr to set
    let new_contract_addr = deps.api.addr_validate(&res.contract_address)?;

    // grab temp lp info
    let mut lp_info = TEMP_LP_INFO.load(deps.storage)?;

    let mut messages = vec![];
    let mut submessages = vec![];
    // update the correct contract
    match id {
        CLP_INSTANTIATE_REPLY_ID => {
            if lp_info.clp_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.clp_contract = new_contract_addr;
        }
        PLP_INSTANTIATE_REPLY_ID => {
            if lp_info.plp_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.plp_contract = new_contract_addr;
        }
        YLP_INSTANTIATE_REPLY_ID => {
            if lp_info.ylp_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.ylp_contract = new_contract_addr.clone();
            let cfg = CONFIG.load(deps.storage)?;

            // update lp_info collector
            // this field exists in case there needs to be a collector per yLP
            lp_info.collector = cfg.collector.clone();

            let collector_config = query_collector_config(&deps.querier, cfg.collector.clone())?;
            let prism_token = collector_config.prism_token;

            // create yasset-staking and yasset-x-staking contracts
            submessages.push(SubMsg {
                msg: WasmMsg::Instantiate {
                    code_id: cfg.yasset_contract_id,
                    msg: to_binary(&YassetInstantiateMsg {
                        yasset_token: new_contract_addr.clone().into_string(),
                    })?,
                    funds: vec![],
                    admin: None,
                    label: String::from("Prism yLP staking contract"),
                }
                .into(),
                id: YASSET_STAKING_REPLY_ID,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            });
            submessages.push(SubMsg {
                msg: WasmMsg::Instantiate {
                    code_id: cfg.yasset_x_contract_id,
                    msg: to_binary(&YassetXInstantiateMsg {
                        yasset_token: new_contract_addr.into_string(),
                        prism_token,
                        prism_yasset_pair: cfg.prism_yasset_pair.into_string(),
                        collector: cfg.collector.into_string(),
                        token_code_id: cfg.token_code_id,
                    })?,
                    funds: vec![],
                    admin: None,
                    label: String::from("Prism yLP autocompounding contract"),
                }
                .into(),
                id: YASSET_X_STAKING_REPLY_ID,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            });
        }
        YASSET_STAKING_REPLY_ID => {
            if lp_info.yasset_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.yasset_contract = new_contract_addr;
        }
        YASSET_X_STAKING_REPLY_ID => {
            if lp_info.yasset_x_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.yasset_x_contract = new_contract_addr;

            // create the correct vault contract
            let vault_msg = match lp_info.amm {
                1 => create_astroport_vault_msg(deps.as_ref(), env, lp_info.clone()),
                2 => create_terraswap_vault_msg(deps.as_ref(), env, lp_info.clone()),
                _ => Err(ContractError::AmmNotSupported {}),
            }?;
            submessages.push(vault_msg);
        }
        LP_VAULT_REPLY_ID => {
            if lp_info.vault != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.vault = new_contract_addr;

            // create the correct reward dist contract
            let reward_dist_submsg = match lp_info.amm {
                1 => create_astroport_reward_dist_msg(deps.as_ref(), lp_info.clone()),
                2 => create_terraswap_reward_dist_msg(deps.as_ref(), lp_info.clone()),
                _ => Err(ContractError::AmmNotSupported {}),
            }?;
            submessages.push(reward_dist_submsg);
        }
        REWARD_DIST_INSTANTIATE_REPLY_ID => {
            if lp_info.reward_dist_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.reward_dist_contract = new_contract_addr;

            // update the correct vault's reward dist contract
            let update_vault_reward_dist_msg = match lp_info.amm {
                1 => update_astroport_vault_reward_dist(lp_info.clone()),
                2 => update_terraswap_vault_reward_dist(lp_info.clone()),
                _ => Err(ContractError::AmmNotSupported {}),
            }?;
            messages.push(update_vault_reward_dist_msg);

            // everything has been instantiated at this point, so add to state
            VAULTS.save(deps.storage, &lp_info.lp, &lp_info)?;
        }
        _ => {
            return Err(ContractError::InvalidReplyID {});
        }
    };

    // save new temp info
    TEMP_LP_INFO.save(deps.storage, &lp_info)?;
    Ok(Response::new()
        .add_messages(messages)
        .add_submessages(submessages))
}

pub fn create_astroport_vault_msg(
    deps: Deps,
    env: Env,
    lp_info: LPContracts,
) -> ContractResult<SubMsg> {
    let astro_cfg = ASTRO_CONFIG.load(deps.storage)?;
    // create astroport_lp_vault contract
    Ok(SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: astro_cfg.lp_astro_vault_id,
            msg: to_binary(&AstroVaultInstantiateMsg {
                owner: env.contract.address.into_string(),
                generator: astro_cfg.generator.into_string(),
                factory: astro_cfg.factory.into_string(),
                fee: Decimal::percent(15),
                lp_contract: lp_info.lp.clone().into_string(),
                clp_contract: lp_info.clp_contract.clone().into_string(),
                plp_contract: lp_info.plp_contract.clone().into_string(),
                ylp_contract: lp_info.ylp_contract.into_string(),
            })?,
            funds: vec![],
            admin: None,
            label: String::from("Prism astroport lp vault contract"),
        }
        .into(),
        id: LP_VAULT_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    })
}

pub fn create_terraswap_vault_msg(
    deps: Deps,
    env: Env,
    lp_info: LPContracts,
) -> ContractResult<SubMsg> {
    let terra_cfg = TERRASWAP_CONFIG.load(deps.storage)?;
    // create astroport_lp_vault contract
    Ok(SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: terra_cfg.lp_terraswap_vault_id,
            msg: to_binary(&TerraswapVaultInstantiateMsg {
                owner: env.contract.address.into_string(),
                factory: terra_cfg.factory.into_string(),
                fee: Decimal::percent(15),
                lp_contract: lp_info.lp.clone().into_string(),
                clp_contract: lp_info.clp_contract.clone().into_string(),
                plp_contract: lp_info.plp_contract.clone().into_string(),
                ylp_contract: lp_info.ylp_contract.into_string(),
            })?,
            funds: vec![],
            admin: None,
            label: String::from("Prism terraswap lp vault contract"),
        }
        .into(),
        id: LP_VAULT_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    })
}

pub fn create_astroport_reward_dist_msg(deps: Deps, lp_info: LPContracts) -> ContractResult<SubMsg> {
    // grab relevant asset infos
    let mut whitelisted_asset_infos = vec![];
    let generator_assets = query_generator_rewards(deps, &deps.querier, lp_info.lp.clone())?;
    for info in generator_assets {
        whitelisted_asset_infos.push(info);
    }

    let amm_info = query_astroport_pair_info(deps, &deps.querier, lp_info.lp.clone())?;
    for info in amm_info.asset_infos {
        whitelisted_asset_infos.push(info);
    }

    let cfg = CONFIG.load(deps.storage)?;
    // create reward_distribution contract
    Ok(SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: cfg.reward_dist_contract_id,
            msg: to_binary(&RewardDistInstantiateMsg {
                vault: lp_info.vault.clone().into_string(),
                gov: cfg.gov.into_string(),
                yasset_token: lp_info.ylp_contract.clone().into_string(),
                yasset_staking: lp_info.yasset_contract.clone().into_string(),
                yasset_staking_x: lp_info.yasset_x_contract.clone().into_string(),
                collector: lp_info.collector.into_string(),
                delegator_reward_denom: String::new(), // don't need a delegator denom for LP's
                protocol_fee: Decimal::percent(15),
                whitelisted_assets: whitelisted_asset_infos,
            })?,
            funds: vec![],
            admin: None,
            label: String::from("Prism reward distribution contract"),
        }
        .into(),
        id: REWARD_DIST_INSTANTIATE_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    })
}

pub fn create_terraswap_reward_dist_msg(deps: Deps, lp_info: LPContracts) -> ContractResult<SubMsg> {
    // grab relevant asset infos
    // convert terraswap asset infos into astroport type asset infos for now because thats what reward dist requires
    // gross
    let mut whitelisted_asset_infos: Vec<AstroAssetInfo> = vec![];
    let amm_info = query_terraswap_pair_info(deps, &deps.querier, lp_info.lp.clone())?;
    for info in amm_info.asset_infos {
        match info {
            TerraAssetInfo::Token { contract_addr, .. } => {
                whitelisted_asset_infos.push(AstroAssetInfo::Token {
                    contract_addr: deps.api.addr_validate(&contract_addr.clone())?,
                })
            }
            TerraAssetInfo::NativeToken { denom, .. } => {
                whitelisted_asset_infos.push(AstroAssetInfo::NativeToken {
                    denom: denom.clone(),
                })
            }
        }
    }

    let cfg = CONFIG.load(deps.storage)?;
    // create reward_distribution contract
    Ok(SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: cfg.reward_dist_contract_id,
            msg: to_binary(&RewardDistInstantiateMsg {
                vault: lp_info.vault.clone().into_string(),
                gov: cfg.gov.into_string(),
                yasset_token: lp_info.ylp_contract.clone().into_string(),
                yasset_staking: lp_info.yasset_contract.clone().into_string(),
                yasset_staking_x: lp_info.yasset_x_contract.clone().into_string(),
                collector: lp_info.collector.into_string(),
                delegator_reward_denom: String::new(), // don't need a delegator denom for LP's
                protocol_fee: Decimal::percent(15),
                whitelisted_assets: whitelisted_asset_infos,
            })?,
            funds: vec![],
            admin: None,
            label: String::from("Prism reward distribution contract"),
        }
        .into(),
        id: REWARD_DIST_INSTANTIATE_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    })
}

pub fn update_astroport_vault_reward_dist(lp_info: LPContracts) -> ContractResult<CosmosMsg> {
    // update astroport_lp_vault config's reward_dist
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.vault.clone().into_string(),
        msg: to_binary(&AstroVaultExecuteMsg::UpdateConfig {
            owner: None,
            generator: None,
            factory: None,
            reward_dist: Some(lp_info.reward_dist_contract),
            fee: None,
        })?,
        funds: vec![],
    }))
}

pub fn update_terraswap_vault_reward_dist(lp_info: LPContracts) -> ContractResult<CosmosMsg> {
    // update terraswap_lp_vault config's reward_dist
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_info.vault.clone().into_string(),
        msg: to_binary(&TerraswapVaultExecuteMsg::UpdateConfig {
            owner: None,
            factory: None,
            reward_dist: Some(lp_info.reward_dist_contract),
            fee: None,
        })?,
        funds: vec![],
    }))
}

pub fn format_token_name(name: &str, option: String) -> ContractResult<String> {
    // "{}-{}-LP" --> "{}-{}-[c/p/y]LP"
    let index = name.rfind('-');
    if index == None {
        return Err(ContractError::ParseError {});
    }

    let mut test = name.to_string();
    test.insert_str(index.unwrap(), &option);
    Ok(test)
}

pub fn format_token_symbol(symbol: &str, option: String) -> ContractResult<String> {
    // "uLP" --> "[c/p/y]uLP"
    Ok(option + symbol)
}
