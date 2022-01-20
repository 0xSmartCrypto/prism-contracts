# Prism Vault

This contract provides users the ability to bond and unbond yield-generating assets in return for newly minted c-assets or their corresponding p-asset/y-asset pair. The yield-bearing assets are immediately bonded/unbonded with a validator. Unbonding is subject to the standard 21-day unbonding period.  Delegator rewards are distributed to the [yasset-staking](/contracts/prism-yasset-staking) contract which handles reward distribution for y-asset stakers. Airdrop rewards are claimed by this contract and they are also sent to the y-asset staking contract. Additional functionality provided by this contract includes spliting/merging c-assets and p-asset/y-asset pairs, logic for properly handling slashing events, and validator whitelisting.

## ExecuteMsg:
  - **Bond** : Bond a yield bearing asset in return for a newly minted equivalent amount of the underlying c-asset.  The bonded amount is immediately delegated to a validator. 
  - **Unbond** (Cw20 receive hook): Unbond by passing in the corresponding c-asset token in return for the same amount of the previously bonded asset. The underlying yield-bearing asset is undelegated and the c-asset is immediately burned.  Note that the underlying bonded asset is subject to a 21-day holding period and the user must call WithdrawUnbonded in order to receive the underlying after the unbonding period ends.  
  - **BondSplit**: Bond a yield-bearing asset in return for an equivalent amount of it's corresponding p-asset/y-asset pair.  
  - **WithdrawUnbonded**: Withdraw any previously unbonded assets after the unbonding period has expired.  
  - **RegisterValidator**:  Register a validator to be included in the supported validator list.  If a user specifies a validator in either the Bond or the BondSplit message, it must be on the list of supported validators.  
  - **UpdateGlobalIndex**: Withdraws delegator rewards and instructs the [yasset-staking](/contracts/prism-yasset-staking) contract to process those rewards.
  - **DeregisterValidator**: Deregister a validator so that it is removed from the supported validator list.   
  - **CheckSlashing**: Check for slashing events and adjust the exchange rate accordingly based on the slashed amount. When slashing occurs, the exchange rate (total bonded / total issued) drops below 1.  When this happens, all bonding/unbonding operations are subject to a peg recovery fee which will eventually result in the exchange rate converging back to 1.
  - **UpdateParams**: Update general configuration parameters.  Admin only.
  - **UpdateConfig**: Update owner and token contracts.  Admin only.
  - **ClaimAirdrop**: Airdrop claims originate from the [airdrop-registry](/contracts/prism-airdrop-registry) contract, which calls the ClaimAirdrop message on this contract.  We execute the claim here and then send those rewards directly to the [yasset-staking](/contracts/prism-yasset-staking) contract (via DepositRewards) for further reward processing.
  - **Split**: Split a c-asset into it's correspoinding p-asset/y-asset pair.  This burns the c-asset and mints the equivalent amount of the p-asset/y-asset tokens. 
  - **Merge**: Merge a p-asset/y-asset pair.  This burns the p-asset/y-asset pair and mints the c-asset.  
  - **DepositAirdropReward**: Deposits the airdrop reward to the [yasset-staking](/contracts/prism-yasset-staking) contract.  

## QueryMsg:
  - **Config**: Retrives contract configuration paraameters. 
  - **State**: Retrieves state configuration parameters.
  - **CurrentBatch**: Queries the current batch, which contains the batch id and the total amount of unbonding requested in the current batch.  
  - **WhitelistedValidators**: Return list of whitelisted validators. 
  - **WithdrawableUnbonded**: Query the unbonded amount that a user is able to currently withdraw.  
  - **Parameters**: Retrieves more configuration parameters
  - **UnbondRequests**: Query all of the outstanding unbond requests for the specified user. 
  - **AllHistory**: Query all of the unbond history for all users. 
