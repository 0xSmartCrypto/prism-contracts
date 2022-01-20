# Prism Yasset Staking

This contract provides functionality for staking y-assets, as well as calculating and distributing rewards for those staked assets. Delegator rewards from the [vault](contracts/prism-vault) contract are withdrawn directly to this contract. In order to receive delegator and airdrop rewards on a bonded asset, users must stake the corresponding y-asset with this contract. All delegator rewards are swapped for luna, which is then converted to pluna/yluna and deposited into the reward pool for stakers. Airdrop rewards are deposited directly into the reward pool from the vault contract. Stakers have the option of staking in "default" or "xprism" staking modes. In the default mode, rewards are sent as-is directly to the user. In prism mode, rewards are automatically converted to PRISM before sending to the staker. Any rewards accruing from unstaked y-assets are sent to the [collector](/contracts/prism-collector) contract, where they are converted to PRISM and then sent to the [gov](/contracts/prism-gov) contract for distribution among the xPRISM stakers.  

## ExecuteMsg:
  - **Bond** (Cw20 receive hook): Bond a y-asset.
  - **Unbond**: Unbond a y-asset.  There is no unbonding period, y-assets are immediately transferred back to user.  
  - **ClaimRewards**: Claim rewards for the sender address, where the rewards are either sent directly to the claimer (default staking mode), or converted to PRISM and then sent to the claimer (xprism staking mode). 
  - **DepositRewards**: Deposit assets, where 90% is to be allocated to the stakers reward pool, and the remaining 10% retained as a protocol fee and sent to the [collector](../prism-collector) contract.  This method is called for both delegator reward and airdrop reward processing.   
  - **ProcessDelegatorRewards**: Swap our native token balances (received as delegator rewards) for luna, then issue the LunaToPylunaHook message.
  - **LunaToPylunaHook**: Split our entire luna balance into pluna/yluna, and then issue the DepositMintedPylunaHook operation.
  - **DepositMintedPylunaHook**: Issue the DepositRewards message using our entire yluna and pluna balance as assets.  
  - **WhitelistRewardAsset**: Add an asset to the list of supported reward assets.  Only supports token assets (not native), and can only be called by governance contract.  

## QueryMsg:
  - **Config**: Retrives contract configuration paraameters. 
  - **PoolInfo**: Query rward pool information for the specified asset.  
  - **RewardAssetWhitelist**: Query whitelisted reward assets.  
  - **RewardInfo**: Query reward information for the specified staker.  
