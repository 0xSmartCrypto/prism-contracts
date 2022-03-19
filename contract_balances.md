# Contract balances 
This page contains a listing of the balances stored within each contract

## prism-airdrop-registry
| Denom | Description |
| - | - |
| NONE | - |

## prism-basset-vault

| Denom | Description |
| - | - |
| basset | all bassets currently bonded |
| casset |  the vault holds all cAssets obtained from user's splitting their cAsset into pAsset/yAsset |

## prism-collector

| Denom | Description |
| - | - |
| all prismswap assets (uluna, uusd, cluna, yluna, pluna, prism, xprism) | prismwap fees |
| pluna, yluna | protocol fees from yasset-staking |
| airdrops (anc, vkr) | protocol fees from yasset-staking |

## prism-gov

| Denom | Description |
| - | - |
| prism | staked prism |
| xprism | pending redeems - when a user redeems, we hold this xPrism until the unbonding period expires (21 days), then we burn it |

## prism-launch-pool

| Denom | Description |
| - | - |
| prism | reward distribution pool |
| yluna | staked from yield farmers |

## prism-lp-staking

| Denom | Description |
| - | - |
| prism | reward distribution pool |
| lp tokens (any supported lp token) | staked tokens from yield farmers |

## prism-reward-distribution

| Denom | Description |
| - | - |
| NONE | - |

## prism-vault
| Denom | Description |
| - | - |
| cluna | the vault holds all cLuna obtained from user's splitting their cLuna into pLuna/yLuna |
| luna | all undelegated Luna that has not yet been claim by users is stored here |

## prism-yasset-staking
| Denom | Description |
| - | - |
| yluna | staked by protocol users |
| yluna, pluna | delegator rewards that have already been converted to pluna/yluna via ProcessDelegatorRewards |
| native coins |  delegator rewards that have not yet been converted to pluna/yluna.  when we bond/unbond with any validator, delegator rewards are automatically pulled from that validator and sent to yasset-staking.  These are not converted to pluna/yluna until UpdateGlobalIndex is called |
| airdrops |  Deposited from vault |

## prism-yasset-staking-x
| Denom | Description |
| - | - |
| yAsset | staked by protocol users and accumulated from reward deposits that are immediately converted to yAssets |

## prism-xprism-boost
| Denom | Description |
| - | - |
| xprism | the contract holds the xprism locked by all users farming amps |