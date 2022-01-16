# Prism LP-Vault

This contract is responsible for facilitating the bonding, refracting and staking of Astroport LP tokens. This contract is generalized for
any current and future Astroport LP tokens, but currently no other AMM's.

## CW20HookMsg:
- **Bond**: Attempts to place a provided Astroport LP token into the Astro Generator and converts the provided LP token into a corresponding cLP token. This method also contains logic to instantiate the c/p/yLP token sets if the LP token had previously not been seen. 
- **Unbond**: Attempts to withdraw LP tokens from the Astro Generator and return LP tokens to the user. Burns the provided cLP tokens.
- **Stake**: Takes a users yLP token and stakes it in the lp-vault. Once staked, users are eligible to claim relevant generator and AMM rewards corresponding to their share of yLP.

## ExecuteMsg:
- **UpdateConfig**: Only executable by contract owner. Updates config.
- **Split**: Burns cLP tokens and mints p/yLP tokens for user.
- **Merge**: Burns p/yLP tokens and mints cLP tokens for user.
- **Unstake**: Returns users' previously staked yLP tokens.
- **UpdateStakingMode**: Allows users to update their staking mode. Currently supports Default and XPrism mode.
- **ClaimRewards**: Allows a user to claim accrued rewards for all staked yLP tokens.
- **Mint**: Only callable by contract. Mints cLP to user and handles state.
- **Burn**: Only callable by contract. Burns cLP from user and handles state.
- **CreateTokens**: Only callable by contract. Creates c/p/yLP tokens for a corresponding LP token.
- **UpdateLPRewards**: Only callable by contract. Updates the generator and AMM rewards for all stakers of a provided LP token.
- **SendStakerRewards**: Only callable by contract. Sends all accrued rewards to a provided staker.
- **UpdateStakerInfo**: Only callable by contract. Updates staked amount of a staker. 

## QueryMsg:
- **Config**: Retrieves configuration information for this contract.