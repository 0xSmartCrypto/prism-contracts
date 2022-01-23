# Prism Terraswap-LP-Vault

This contract is responsible for facilitating the bonding, refracting and staking of a single Terraswap LP token.
One copy of this contract is instantiated for each Terraswap LP pair.

## CW20HookMsg:
- **Bond**: Converts the provided Terraswap LP token into a corresponding cLP token.

## ExecuteMsg:
- **UpdateConfig**: Only executable by contract owner. Updates config.
- **Unbond**: Attempts to withdraw LP tokens from the Astro Generator and return LP tokens to the user. Burns the provided cLP tokens.
- **Split**: Burns cLP tokens and mints p/yLP tokens for user.
- **Merge**: Burns p/yLP tokens and mints cLP tokens for user.
- **UpdateGlobalIndex**: Collects all rewards and sends to the relevant reward-distribution contracts

## QueryMsg:
- **Config**: Retrieves configuration information for this contract.