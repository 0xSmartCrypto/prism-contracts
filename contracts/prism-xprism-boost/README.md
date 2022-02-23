# Prism Xprism-Boost

This contract is responsible for the pledging/unpledging of users' xprism for boost accrual.
Boost is accrued continuously and lazily updated whenever a specific user's boost is requested.

## Cw20HookMsg:
- **Bond**: Only callable by xprism token, lets a user bond xprism and begin accruing amps.

## ExecuteMsg:
- **UpdateConfig**: Only callable by owner. Updates config.
- **Unbond**: Allows user to unbond already pledged xprism, resets a user's boost to 0 on any size withdraw.

## QueryMsg:
- **Config**: Retrieves configuration information for this contract.
- **GetBoost**: Retrieves info about a user with bonded xprism.