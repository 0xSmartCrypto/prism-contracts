# Prism LP Staking

This contract provides functionality for staking one of the supported lp tokens in return for PRISM reward tokens. The contract is initialized with a list of supported staking tokens and a distribution schedule which specifies the amount of PRISM that is to be pro-rata distributed to stakers over specific time intervals.

## ExecuteMsg:
  - **Bond** (Cw20 receive hook): This method bonds lp tokens in return for LP rewards in the form of PRISM tokens.
  - **Unbond**: Unbond previously bonded lp tokens.  There is no unbonding period, the corresponding lp tokens are transferred back to the user immediately.
  - **ClaimRewards**: Computes rewards for a given user and lp token and sends those rewards to the user.   

## QueryMsg:
  - **Config**: Retrives contract configuration.
  - **PoolInfo**: Retrieves pool info for the specified lp staking token.  This contains the weight for this lp token, the total bonded amount, and information related to reward computations.  
  - **StakerInfo**: Retrieves staking information for a given staker.  This includes bond amounts for all of the tokens that this user has staked, along with any pending unclaimed rewards.  
  - **TokenStakersInfo**: Retrieves StakerInfo for every staker of a given lp token.  
