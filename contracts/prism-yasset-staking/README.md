# Prism Yasset Staking

This contract provides functionality for staking y-assets and claiming rewards for those staked assets. Rewards from the [reward-distribution](contracts/prism-reward-distribution) contract are deposited into this contract where they can be claimed by the stakers.   

For each reward asset, we maintain a PoolInfo object, which contains a single "reward_index" field.  This field represents an accumulating global reward value for each reward asset, normalized by the total bonded y-asset amount.  On every reward deposit and for each reward asset, we compute the *normal_reward_per_bond = (asset reward amount / total y-asset bonded amount)*, and then we increment the PoolInfo's reward_index field by this normal_reward_per_bond value.

For each user and for each reward asset, we maintain a RewardInfo object, which contains the "index" and "pending_reward" fields.  The index field represents a checkpoint of the corresponding PoolInfo's reward_index field at the time of last update, and the pending rewards represents the accumulated rewards since rewards were last claimed.  Whenever a staker's bonded amount changes (via Bond/Unbond), we want to capture the accumulated rewards from the last update and checkpoint this reward attribution.  We do this by computing the prior segment's pending_reward value to be *(staker's bond_amount * pool_info.reward_index) - (staker's bond_amount * user's reward_info.index)*.  This captures the amount of rewards that the user has accumulated since the last checkpoint.  We then update the user's reward_info.index field for this asset to the corresonding PoolInfo's reward_index value, and we increment the user's reward_info.pending_reward by the prior segment's accumulated rewards.  We then update the user's bonded amount and a new reward segment begins for this user with the new bonded amount.

When rewards are claimed, we go through the same process of updating the staker's RewardInfo objects.  This brings the pending_reward information up-to-date with the current block.  We then zero out the pending rewards and distribute the rewards to the staker.

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
