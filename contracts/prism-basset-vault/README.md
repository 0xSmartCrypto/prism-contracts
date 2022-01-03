# Prism Basset Vault

This contract provides users the ability to bond and unbond b-assets (bETH, bSOL) in return for newly minted c-assets or their corresponding p-asset/y-asset pair. Rewards are periodically claimed (inside UpdateGlobalIndex) from the b-asset contracts and sent to the [reward-distribution](/contracts/prism-reward-distribution) contract which handles reward distribution for y-asset stakers. This contract also provides functionality to split/merge c-assets and their p-asset/y-asset pairs.

## ExecuteMsg:
  - **Bond** (Cw20 receive hook): Bond a b-asset in return for it's associated c-asset, this is always a 1-1 exchange. 
  - **Unbond** (Cw20 receive hook): Unbond by passing in the corresponding c-asset token in return for the same amount of the previously bonded asset. There is no unbonding period, b-assets are returned immediately.  
  - **BondSplit** (Cw20 receive hook): Bond a b-asset in return for an equivalent amount of it's corresponding p-asset/y-asset pair.  
  - **Split**: Split a c-asset into it's correspoinding p-asset/y-asset pair.  This burns the c-asset and mints the equivalent amount of the p-asset/y-asset tokens. Requires prior  IncreaseAllowance call on c-asset token contract.  
  - **Merge**: Merge a p-asset/y-asset pair.  This burns the p-asset/y-asset pair and mints the c-asset.  Requires prior IncreaseAllowance call on p-asset and y-asset token contracts.  
  - **UpdateGlobalIndex**: Claims rewards from underlying b-asset contracts and distributes those rewards to the [reward-distribution](/contracts/prism-reward-distribution) contract.
  - **UpdateConfig**: Update owner and token/distribution contracts.  Admin only.

## QueryMsg:
  - **Config**: Retrieves contract configuration paraameters. 
  - **State**: Retrieves state configuration parameters.
  - **BondedAmount**: Queries the current bonded amount of b-assets.  
