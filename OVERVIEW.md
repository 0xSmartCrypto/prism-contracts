# Prism Overview

PRISM litepaper: https://prismfinance.app/PRISM-litepaper.pdf

# Contracts

## [prism-vault](/contracts/prism-vault):
This contract provides users the ability to bond and unbond yield-generating assets in return for newly minted c-assets or their corresponding p-asset/y-asset pair. The yield-bearing assets are immediately bonded/unbonded with a validator. Unbonding is subject to the standard 21-day unbonding period.  Delegator rewards are distributed to the [yasset-staking](/contracts/prism-yasset-staking) contract which handles reward distribution for y-asset stakers. Airdrop rewards are claimed by this contract and they are also sent to the y-asset staking contract. Additional functionality provided by this contract includes spliting/merging c-assets and p-asset/y-asset pairs, logic for properly handling slashing events, and validator whitelisting.

## [prism-yasset-staking](/contracts/prism-yasset-staking):
This contract provides functionality for staking y-assets, as well as calculating and distributing rewards for those staked assets. Delegator rewards from the [vault](contracts/prism-vault) contract are withdrawn directly to this contract. In order to receive delegator and airdrop rewards on a bonded asset, users must stake the corresponding y-asset with this contract. All delegator rewards are swapped for luna, which is then converted to pluna/yluna and deposited into the reward pool for stakers. Airdrop rewards are deposited directly into the reward pool from the vault contract. Stakers have the option of staking in "default" or "xprism" staking modes. In the default mode, rewards are sent as-is directly to the user. In prism mode, rewards are automatically converted to PRISM before sending to the staker. Any rewards accruing from unstaked y-assets are sent to the [collector](/contracts/prism-collector) contract, where they are converted to PRISM and then sent to the [gov](/contracts/prism-gov) contract for distribution among the xPRISM stakers.  

## [prism-collector](/contracts/prism-collector):
This contract is responsible for collecting protocol fee rewards and unstaked y-asset delegator rewards from the yasset-staking contract, converting those rewards to PRISM, and sending those PRISM tokens to the [gov](/contracts/prism-gov) contract. Additionally, when the yasset staker has elected to stake with xprism staking mode, this contract provides functionality to swap input assets to PRISM and send them back to the specified receiver.

## [prism-gov](/contracts/prism-gov):
This contract provides standard governance polling/voting functionality as well as support for minting/redeeming xPRISM and staking xPRISM for voting rights. The redemption process for xPRISM is subject to a 21-day holding period. Only staked xPRISM holders are allowed to vote, and each staked xPRISM token allows for one vote. The PRISM/xPRISM relationship is similar to UST/aUST, where the xPRISM is constantly appreciating in value with respect to PRISM. This appreciation is due to the protocol fees from the yasset-staking contract being converted to PRISM (inside the collector contract) and then sent to us, which is then included in our PRISM/xPRISM exchange rate calculation. Polling and voting logic works similar to the anchor protocol: https://docs.anchorprotocol.com/protocol/anchor-governance.

## [prism-lp-staking](/contracts/prism-lp-staking):
This contract provides functionality for staking one of the supported lp tokens in return for PRISM reward tokens. The contract is initialized with a list of supported staking tokens and a distribution schedule which specifies the amount of PRISM that is to be pro-rata distributed to stakers over specific time intervals.

## [prism-limit-order](/contracts/prism-limit-order):
This contract provides limit order functionality for AMM trading pairs. Users submit "orders" containing an offer (sell) asset and an ask (buy) asset where they require a certain ask asset amount in return for their provided offer asset amount. Executions occur through external users/bots submitting ExecuteOrder messages on these limit orders when conditions are favorable. The user executing the order is rewarded with a percentage of the fee and a portion of any excess ask asset amount captured from the swap. The protocol also captures a fee which is sent to the [gov](/contracts/prism-gov) contract to reward PRISM stakers. Note that PRISM can be used as an intermediate swap pair in the event that the specified offer/ask trading pair is not directly available.  

## [prism-airdrop-registry](/contracts/prism-airdrop-registry):
This contract stores airdrop information and provides the ability to initiate airdrop claims. Admin users will submit a claim for an airdrop with the associated proof.  This contract then creates an airdrop claim message and submits a ClaimAirdrop message on the [vault](/contracts/prism-vault) contract, which will execute the claim and deposit the airdrop rewards to the [yasset-staking](/contracts/prism-yasset-staking) contract.

## [prism-fair-launch](/contracts/prism-fair-launch):
This contract provides functionality for executing a "fair launch" for distribution of the initial PRISM tokens. This consists of a two phase auction for the tokens. During Phase 1, users can deposit and withdraw any amount of uusd. During Phase 2, users can only withdraw tokens. After Phase 2, users can withdraw their pro-rata allocated portion of the distributed PRISM tokens.

## [prism-launch-pool](/contracts/prism-fair-launch):
This contract provides functionality for the community farming launch event. We initialize this contract with a distribution schedule which specifies the amount of PRISM that is to be distributed linearly over the entire farming interval. Users bond their y-asset tokens with this contract in return for PRISM tokens, and users can unbond their tokens at any time without penalty. PRISM rewards are subject to a 21-day vesting period from the time that their withdrawal is requested. Staking rewards from the bonded y-asset tokens are periodically claimed and sent to the contract owner (PRISM labs).
