# Prism Delegator Rewards

This contract provides functionality for converting delegator rewards from the luna vault into pLuna/yLuna and distributing that pLuna/yLuna as rewards to the reward distribution contract.  This contract is only used by the prism-vault (luna) contract.

## ExecuteMsg:
| Message | Privileges | Description |
| - | - | - |
| **ProcessDelegatorRewards** | vault | Convert all native balances to luna via market swaps and then issue the LunaToPylunaHook operation. | 
| **LunaToPylunaHook** | contract | Split our entire luna balance into pluna/yluna, and then issue the DistributeMintedPylunaHook operation. |
| **DistributeMintedPylunaHook** | contract | Distribute our pLuna/yLuna balance to the  [reward-distribution][1] contracts. |  
| **UpdateConfig** | owner | Update contract configuration parameters |

## QueryMsg:
| Message | Description |
| - | - |
| **Config** | Retrieve contract configuration parameters. |

[1]: /contracts/prism-reward-distribution
