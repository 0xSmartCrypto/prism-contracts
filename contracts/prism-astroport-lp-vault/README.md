# Prism LP-Vault

This contract is responsible for facilitating the bonding and refracting of an Astroport LP token.
One instance of this contract is instantiated for each LP.

## CW20HookMsg:
- **Bond**: Attempts to place a provided Astroport LP token into the Astro Generator and converts the provided LP token into a corresponding cLP token.

## ExecuteMsg:
- **UpdateConfig**: Only executable by contract owner. Updates config.
- **Unbond**: Attempts to withdraw LP tokens from the Astro Generator and return LP tokens to the user. Burns the provided cLP tokens.
- **Split**: Burns cLP tokens and mints p/yLP tokens for user.
- **Merge**: Burns p/yLP tokens and mints cLP tokens for user.
- **UpdateGlobalIndex**: Collects all rewards and sends to the relevant reward-distribution contracts.


## QueryMsg:
- **Config**: Retrieves configuration information for this contract.
- **LPInfo**: Retrieves relevant information for the LP this contract supports.
- **BondedAmount**: Retrieves amount of LP bonded within this contract.