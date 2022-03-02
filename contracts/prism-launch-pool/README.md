# Prism Launch Pool

This contract provides functionality for the community farming launch event. We initialize this contract with a distribution schedule which specifies the amount of PRISM that is to be distributed linearly over the entire farming interval. Users bond their y-asset tokens with this contract in return for PRISM tokens, and users can unbond their tokens at any time without penalty. PRISM rewards are subject to a 30-day vesting period from the time that their withdrawal is requested. Staking rewards from the bonded y-asset tokens are periodically claimed and sent to the contract owner (PRISM labs).

Behind the scenes, this contract takes the y-assets sent by users and stakes them with the [yasset-staking][1] contract to generate yield rewards. These rewards are periodically claimed and sent to this contract's owner (PRISM labs).

You can think about this contract as an alternative to the [yasset-staking][1] contract for users. If Alice has one yluna, she can do at least two different things with it:
1) She can stake in the [yasset-staking][1] contract and get yield rewards over time; or
2) She can stake it in this contract and get PRISM tokens over time.
## ExecuteMsg:
  - **Bond** (Cw20 receive hook): This method bonds y-assets in return for PRISM rewards.  The bonded assets are immediately bonded with the [yasset-staking][1] contract.
  - **Unbond**: Allows users to unbond their previously bonded y-assets.  There is no unbonding period, the corresponding y-asset tokens are immediately unstaked from the [yasset-staking][1] contract and transferred back to the user.
  - **WithdrawRewards**: This method initiates the PRISM reward withdrawal process.  After calling this method, users must wait 30 days for their PRISM rewards to become fully vested.  After the 30-day vesting period, the user can call ClaimWithdrawnRewards in order to receive their PRISM rewards.
  - **ClaimWithdrawnRewards**: This method will send all vested PRISM tokens to the claiming user.  WithdrawRewards should be called prior to this method in order to begin the vesting process.
  - **AdminWithdrawRewards**: This method will claim our staking rewards from the [yasset-staking][1] contract and then issue an AdminSendWithdrawnRewards message to send those rewards to the contract owner.  Must be called by Admin user.
  - **AdminSendWithdrawnRewards**:  Sends staking rewards that were previously claimed inside AdminWithdrawRewards to the contract owner.  Must be called by this contract (we call this inside AdminWithdrawRewards).

## QueryMsg:
  - **Config**: Retrieve contract configuration.
  - **DistributionStatus**: Retrieves current reward distribution status, which includes total amount distributed, total bond amount, pending reward, and reward index.
  - **RewardInfo**: Retrieves reward information for a given staker address.
  - **VestingStatus**: Retrieves vestin status information for a given staker address.

[1]: /contracts/prism-yasset-staking