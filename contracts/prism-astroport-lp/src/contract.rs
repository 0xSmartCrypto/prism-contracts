//#[cfg(not(feature = "library"))]
// use cosmwasm_std::entry_point;
// use cosmwasm_std::{
//     from_binary, to_binary, Addr, Binary, CanonicalAddr, Deps, DepsMut, Env, MessageInfo, Response,
// };
// use cw20::Cw20ReceiveMsg;

// use crate::error::ContractError;

// clarify:
// 1. what is a "c-asset"? collat?
// 6. yLP holders or stakers?
// 6.2. what is an "xyLP token/pool"? shouldn't an autocompounder just convert to yLP and immediately stake it on behalf of the owner? does this xyLP token/pool exist somewhere or not built yet?
// 6.2. could this "autocompounder" be added into yasset-staking functionality? e.g. 1. native, 2. xprism, 3. autocompound
// -. what are collateral tokens? what would they be used for?

// notes:
// 1. use vault to split astroport LP tokens into p/y-LP
// 2. use astro-generator-proxy to stake received astroport LP tokens
// 3. issue p/y-LP tokens after SUCCESSFULLY staking LP tokens into astro-generator
// 4. assuming astro generator will pay liquidity incentive rewards directly into vault contract wallet
// 5. calculate AMM fees since last collection (might be on astroport's chain), burn corresponding number of LP tokens into vault contract
// 6. distribute rewards to yLP holders (holders or stakers?) if stakers, we can make use of yasset-staking for 6.1 and 6.3. Clarify 6.2.
// 7.1. collect 15% of yield from astroport LP's in vault as fees and distribute to yLP stakers (holders?)
// 7.2. collect 15% of yield from "autocompounder" (clarify 6.2) and distribute to xPrism pool via collector
// 7.3. collect 100% of yield from yLP not being staked or autocompounded and distribute to xPrism pool via collector
// -. make this extensible to every astroport LP token