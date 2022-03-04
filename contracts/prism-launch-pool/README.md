# Prism Launch Pool

This contract provides functionality for the community farming launch event. Users bond their y-asset tokens with this contract in return for PRISM tokens, and users can unbond their tokens at any time without penalty. PRISM rewards are subject to a 30-day vesting period. This contract takes the y-assets sent by users and stakes them with the [yasset-staking][1] contract to generate yield rewards.  Staking rewards from the bonded y-asset tokens are periodically claimed and sent to the contract owner (PRISM labs).  

PRISM rewards are broken down into two separate reward pools, one for the base rewards and another for the boost (AMP) rewards.  Both pools have a linear distribution schedule throughout the life of the farming contract.  The base pool rewards are distributed based on a user's bonded amount in proportion to the total bonded amount.  The boost pool rewards are distributed proprotionally based on a user's AMP value, which is computed based on the amount of bonded xprism inside the [xprism-boost][2] contract. Each user has a boost weight, which is computed as sqrt(AMP value * y-asset bonded amount).  Each user's boost rewards are computed as as a proportion of their boost weight and the sum of all user's boost weights.  Note that a user's AMP value is not automatically updated, user's must execute an ActivateBoost call in order for the contract to process changes to a user's AMP value.  

You can think about this contract as an alternative to the [yasset-staking][1] contract for users. If Alice has one yluna, she can do at least two different things with it:
1) She can stake in the [yasset-staking][1] contract and get yield rewards over time; or
2) She can stake it in this contract and get PRISM tokens over time.

## ExecuteMsg:
| Message | Privileges | Description |
| - | - | - |
| **Bond** | CW20 receive hook for yluna | This method bonds y-assets in return for PRISM rewards.  The bonded assets are immediately bonded with the [yasset-staking][1] contract. |
| **Unbond** | | Allows users to unbond their previously bonded y-assets.  There is no unbonding period, the corresponding y-asset tokens are immediately unstaked from the [yasset-staking][1] contract and transferred back to the user. |
| **ActivateBoost** | | This method retrieves a user's current AMP value from the [xprism-boost][2] contract, recomputes their boost weight, and updates the contract's total boost weight.  This method should be called periodically by individual users in order to pick up any recently accumulated AMP value. |
| **WithdrawRewards** | | This method initiates the PRISM reward withdrawal process.  After calling this method, users must wait 30 days for their PRISM rewards to become fully vested.  After the 30-day vesting period, the user can call ClaimWithdrawnRewards in order to receive their PRISM rewards.  Note that it's not necessary for users to call this directly, as the WithdrawRewardsBulk method is called by a bot on a daily basis. |
| **WithdrawRewardsBulk** | operator | This method withdraws rewards for each individual user and is initiated by a bot on a daily basis.  Due to gas restrictions, we limit the number of withdraws performed during a single execution, and we provide a form of pagination where we return the last updated address from the call so that the bot knows where to start on the next call. | 
| **ClaimWithdrawnRewards** | | This method allows users to claim their vested rewards and provides several options for how the users would like to receive their rewards.  Available options are PRISM, xPRISM (PRISM rewards converted to xPRISM via gov contract), and AMPS (PRISM rewards converted to xPRISM via gov contract and then bonded with [xprism-boost contract][2]). |
| **BondWithBoostContractHook** | contract | This hook is called when claiming rewards using the AMPS option, and is used to bond a user's xPRISM rewards with the [xprism-boost][2] contract. |
| **AdminWithdrawRewards** | owner | This method will claim our staking rewards from the [yasset-staking][1] contract and then issue an AdminSendWithdrawnRewards message to send those rewards to the contract owner. |
| **AdminSendWithdrawnRewards** | contract | Sends staking rewards that were previously claimed inside AdminWithdrawRewards to the contract owner.  Called from inside AdminWithdrawRewards |

## QueryMsg:
| Message | Description |
| - | - |
| **Config** | Retrieve contract configuration. |
| **DistributionStatus** | Retrieves current reward distribution status, which includes total amount distributed, total bond amount, pending reward, and reward index. |
| **RewardInfo** | Retrieves reward information for a given staker address. |
| **VestingStatus** |  Retrieves vestin status information for a given staker address. |

[1]: /contracts/prism-yasset-staking
[2]: /contracts/prism-xprism-boost
