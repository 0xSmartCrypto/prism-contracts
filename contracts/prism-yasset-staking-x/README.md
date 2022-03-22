# Prism Yasset Staking X

This contract provides auto-compounding functionality for staking y-assets.  Stakers receive an xy-asset in return for their staked y-asset, where the amount of xy-assets minted is computed as the total bonded amount divided by the y-asset balance in this contract.  Rewards from the [reward-distribution](contracts/prism-reward-distribution) contract are deposited into this contract.  These rewards are immediately converted to y-assets via a collector ConvertAndSend message.  This process results in a continuous increase of our y-asset balance thereby appreciating the value of the xy-asset tokens. The corresponding xy-asset token contract is created during instantation of this contract.  

## ExecuteMsg:
| Message | Privileges | Description |
| - | - | - |
| **Bond** | CW20 receive hook for yasset | Bond a y-asset in return for a newly minted xy-asset token, where the minted xy-asset amount is based on the current exchange rate. |
| **Unbond** | Cw20 receive hook for xyasset |: Unbond a y-asset by passing in the corresponding xy-asset.  There is no unbonding period, y-assets are immediately transferred back to user at the current exchange rate. | 
 | **DepositRewards** | reward_distribution contract | Deposit assets, this method is called by the [reward-distribution](contracts/prism-reward-distribution) contract and all of the returns are immediately swapped to the coresponding y-asset.  Deposited assets must either be sent with this message (native assets) or caller must increase the token allowance for this contract (CW20 tokens).  Rewards are swapped to the yasset via the [collector](contracts/prism-collector) contract's ConvertAndSend method. |
| **PostInitialize** | owner | Set the reward distribution contract. |

## QueryMsg:
| Message | Description |
| - | - |
| **Config** | Retrieves contract configuration paraameters. |
| **State** | Query total bond amount, y-asset balance, and current exchange rate (y-asset balance / total bond amount). |
