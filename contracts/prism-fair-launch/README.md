# Prism Fair Launch

This contract provides functionality for executing a "fair launch" for distribution of the initial PRISM tokens. This consists of a two phase auction for the tokens. During Phase 1, users can deposit and withdraw any amount of uusd. During Phase 2, users can only withdraw tokens. After Phase 2, users can withdraw their pro-rata allocated portion of the distributed PRISM tokens.

## ExecuteMsg:
  - **Deposit**: Deposit uusd into this contract, only allowed durin Phase1.  
  - **Withdraw**: Withdraw uusd into this contract, allowed during Phase1 and Phase2.  
  - **WithdrawTokens**: Withdraw pro-rata allocated PRISM tokens, only allowed at the end of the launch (after Phase2).  
  - **PostInitialize**: Initialize the contract's LaunchConfig parameters, which contains the total PRISM distribution amount and the phase start/end timestamps. Must be called by owner.
  - **AdminWithdraw**: Withdraw the contract's uusd balance at the end of the launch.  Must be called by owner.  

## QueryMsg:
  - **Config**:  Retrieves contract configuration paraameters. 
  - **DepositInfo**: Retrieves deposit info for a user, which includes the user's deposit amount and the total deposit amount. 
