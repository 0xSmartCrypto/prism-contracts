#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, SubMsg, Reply, WasmMsg, ReplyOn
};

use prism_protocol::lp_vault_factory::{Config, AstroConfig, LPContracts};
use prism_common::parse_reply_instantiate_data;

use crate::error::{ContractError, ContractResult};
use crate::query::{query_config, query_vault, query_token_info};
use crate::state::{CONFIG, ASTRO_CONFIG, VAULTS, TEMP_LP_INFO};

use cw20::{MinterResponse};
use cw20_base::msg::InstantiateMsg as TokenInstantiateMsg;

use terra_cosmwasm::{TerraMsgWrapper};

const CLP_INSTANTIATE_REPLY_ID: u64 = 1;
const PLP_INSTANTIATE_REPLY_ID: u64 = 2;
const YLP_INSTANTIATE_REPLY_ID: u64 = 3;
const REWARD_DIST_INSTANTIATE_REPLY_ID: u64 = 4;
const YASSET_STAKING_REPLY_ID: u64 = 5;
const YASSET_X_STAKING_REPLY_ID: u64 = 6;
const LP_VAULT_REPLY_ID: u64 = 7;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn create_astroport_vault(
    deps: DepsMut,
    env: Env,
    lp: Addr,
) -> ContractResult<Response<TerraMsgWrapper>> {
    // need to instantiate:
    // c/p/yLP tokens first

    // reward-distribution
    // yasset-staking
    // yasset-x-staking
    // vault
    
    // create c/p/yLP tokens
    let cfg = CONFIG.load(deps.storage)?;
    let token_info = query_token_info(&deps.querier, lp.clone())?;
    let mut submessages = vec![
        SubMsg {
            msg: WasmMsg::Instantiate {
                code_id: cfg.token_code_id.clone(),
                msg: to_binary(&TokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "c".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "c".to_string())?,
                    decimals: token_info.decimals.clone(),
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
                code_id: cfg.token_code_id.clone(),
                msg: to_binary(&TokenInstantiateMsg {
                    name: format_token_name(&token_info.name, "p".to_string())?,
                    symbol: format_token_symbol(&token_info.symbol, "p".to_string())?,
                    decimals: token_info.decimals.clone(),
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

    // yasset-staking next

    // yasset-x-staking next

    // reward-distribution next

    // lp-vault after

    // yasset-staking
    // pub yasset_token: String,

    // yasset-x-staking
    // pub yasset_token: String,
    // pub prism_token: String,
    // pub prism_yasset_pair: String,
    // pub collector: String,
    // pub token_code_id: u64, // cw20 token code id for xyasset token creation
    
    // reward distribution
    // pub vault: String,
    // pub gov: String,
    // pub yasset_token: String,
    // pub yasset_staking: String,
    // pub yasset_staking_x: String,
    // pub collector: String,
    // pub delegator_reward_denom: String,
    // pub protocol_fee: Decimal,
    // pub whitelisted_assets: Vec<AssetInfo> 
    Ok(Response::new().add_submessages(submessages))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn create_terraswap_vault(
    deps: DepsMut,
    lp: Addr,
) -> ContractResult<Response<TerraMsgWrapper>> {
    // need to instantiate:
    // c/p/yLP tokens (look for the new create_tokens stuff)
    // reward-distribution
    // yasset-staking
    // yasset-x-staking
    // vault
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
#[allow(dead_code)] // throws warnings on compile because we don't call reply explicitly
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> ContractResult<Response> {
    let id = msg.id;
    let res = parse_reply_instantiate_data(msg)
        .map_err(|_| ContractError::ParseError {})?;
    
    // get contract addr to set
    let new_contract_addr = deps.api.addr_validate(&res.contract_address)?;

    // grab temp lp info
    let mut lp_info = TEMP_LP_INFO.load(deps.storage)?;

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
            lp_info.ylp_contract = new_contract_addr;
        }
        REWARD_DIST_INSTANTIATE_REPLY_ID => {
            if lp_info.reward_dist_contract != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.reward_dist_contract = new_contract_addr;
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
        }
        LP_VAULT_REPLY_ID => {
            if lp_info.vault != Addr::unchecked("") {
                return Err(ContractError::ReplyErr {});
            }
            lp_info.vault = new_contract_addr;

            // everything has been instantiated at this point, so add to state
            VAULTS.save(deps.storage, (lp_info.amm.into(), &lp_info.lp), &lp_info)?;
        }
        _ => {
            return Err(ContractError::InvalidReplyID {});
        }
    };

    // save new temp info
    TEMP_LP_INFO.save(deps.storage, &lp_info)?;
    Ok(Response::new())
}

// ??
// pain
// QUES: is there any better way to do this string manip
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
