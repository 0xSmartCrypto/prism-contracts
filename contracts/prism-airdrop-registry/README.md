# Prism Airdrop Registry

This contract stores airdrop information and provides the ability to initiate airdrop claims. Admin users will submit a claim for an airdrop with the associated proof.  This contract then creates an airdrop claim message and submits a ClaimAirdrop message on the [vault](/contracts/prism-vault) contract, which will execute the claim and deposit the airdrop rewards to the [yasset-staking](/contracts/prism-yasset-staking) contract.

## ExecuteMsg:
- **FabricateClaim**: Initiate the claim process on a given airdrop token.
- **UpdateConfig**: Update contract configuration, allows contract owner to change either the contract owner or the vault contract.  
- **AddAirdropInfo**: Adds an airdrop token and associated airdrop contract to the supported airdrop list.  
- **RemoveAirdropInfo**: Removes an airdrop token from the supported airdrop list.  
- **UpdateAirdropInfo**: Updates airdrop information for an existing token, or creates new airdrop information if the token wasn't already in the supported airdrop list.    

## QueryMsg:
- **Config**: Retrieves configuration information for this contract, which includes owner, vault contract, and a list of supported airdrop tokens.
- **AirdropInfo**: Retrieves a vector containing information for all the currently supported airdrops.  
