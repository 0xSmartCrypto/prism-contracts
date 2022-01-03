# Prism Yasset Staking

This contract provides functionality for staking y-assets and claiming rewards for those staked assets. Rewards from the [reward-distribution](contracts/prism-reward-distribution) contract are deposited into this contract where they can be claimed by the stakers.   

## ExecuteMsg:
  - **Bond** (Cw20 receive hook): Bond a y-asset.
  - **Unbond**: Unbond a y-asset.  There is no unbonding period, y-assets are immediately transferred back to user.  
  - **ClaimRewards**: Claim any accumulated rewards.
  - **DepositRewards**: Deposit assets, this method is called by the [reward-distribution](contracts/prism-reward-distribution) contract.  Deposited assets must either be sent with this message (native assets) or caller must increase the token allowance for this contract (CW20 tokens). 
  - **PostInitialize**: Set the reward distribution contract, must be called by owner.

## QueryMsg:
  - **Config**: Retrieves contract configuration paraameters. 
  - **PoolInfo**: Query reward pool information for the specified asset.  
  - **RewardInfo**: Query reward information for the specified staker.  
  - **State**: Query total bonded amount.  
