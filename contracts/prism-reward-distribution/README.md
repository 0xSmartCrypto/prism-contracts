# Prism Reward Distribution

This contract provides functionality for distributing rewards appropriately to the [yasset-staking](/contracts/prism-yasset-staking), [yasset-staking-x](/contracts/prism-yasset-staking-x), and [collector](/contracts/prism-collector) contracts.  Delegator rewards from [prism-vault](/contracts/prism-vault) are converted to luna prior to distributing to y-asset stakers.  UST rewards from [prism-basset-vault](/contracts/prism-basset-vault) along with any airdrop rewards are distributed directly to the y-asset stakers.  Rewards are distributed pro-rata to the yasset-staking and the yasset-staking-x contracts.  Rewards from any unbonded y-assets are sent to the collector contract, along with a 10% protocol fee on any bonded y-assets.  All reward assets sent from the vault must be whitelisted.  

## ExecuteMsg:
| Message | Privileges | Description |
| - | - | - |
| **DistributeRewards** | | Distribute our current balance of the input reward asset appropriately to the [yasset-staking](/contracts/prism-yasset-staking), [yasset-staking-x](/contracts/prism-yasset-staking-x), and [collector](/contracts/prism-collector) contracts. |
| **WhitelistRewardAsset** | owner | Add an asset to the list of supported reward assets. |
| **RemoveRewardAsset** | owner | Remove a whitelisted reward asset. |   
| **Updateconfig** | owner | Add an asset to the list of supported reward assets. |

## QueryMsg:
| Message | Description |
| - | - |
| **Config** | Retrieves contract configuration paraameters. |
| **RewardAssetWhitelist** | Query whitelisted reward assets. | 
