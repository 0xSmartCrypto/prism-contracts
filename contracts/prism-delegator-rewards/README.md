# Prism Delegator Rewards

This contract provides functionality for converting delegator rewards from the luna vault into pLuna/yLuna and distributing that pLuna/yLuna as rewards to the reward distribution contract.  This contract is only used by the prism-vault (luna) contract.

## ExecuteMsg:
  - **ProcessDelegatorRewards**: Convert all native balances to luna via market swaps and then issue the LunaToPylunaHook operation.  Vault-only. 
  - **LunaToPylunaHook**: Split our entire luna balance into pluna/yluna, and then issue the DistributeMintedPylunaHook operation.  Contract-only. 
  - **DistributeMintedPylunaHook**: Distribute our pLuna/yLuna balance to the  [reward-distribution][1] contracts.  Contract-only.  

## QueryMsg:
  - **Config**: Retrieves contract configuration paraameters. 

[1]: /contracts/prism-reward-distribution
