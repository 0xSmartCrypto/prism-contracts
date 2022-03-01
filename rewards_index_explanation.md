# The Rewards Index Trick™

## DistributionStatus.reward_index
`reward_index` is part of a trick to lazily compute each user's actual earned rewards in an efficient manner.

"Index" is a bit of a misnomer since it doesn't mean the typical "index" of an array. Perhaps a better name
would be `cumulative_piecewise_rewards_per_bonded_unit` (see below).

`reward_index` is a monotonically increasing value (i.e. it only grows, it never decreases). In a nutshell, it is
the sum over piecewise time intervals (up to the current time) of rewards that have been released during every
time interval divided by the corresponding total bond amount during that time interval (see below). Its units
are PRISM tokens per bonded yluna token.

Specifically, `reward_index` starts at 0 and gets incremented any time there is an "event". An "event" here means
any user called bond, unbond or withdraw_rewards. When an event happens, we do this (at the very beginning of
the blockchain transaction):

  - Let T be the time interval elapsed between the current event and the previous event.
  - Let R be the reward that ought to be linearly released during T, according to the contract's schedule (in
    PRISM tokens).
  - Let B be the total amount of bound ylunas among all users during T (in yluna tokens). Note that B is
    guaranteed to be constant throughout T, because by definition there weren't any bond or unbond events during
    T.
  - Increment `reward_index` by R / B (so units are PRISM tokens per bound yluna token).

`reward_index` doesn't make any sense on its own; it is only useful when combined with each user's "index" field
in the RewardInfo struct. When there is an event that involves a specific user (i.e. the user calls bond, unbond
or withdraw_rewards), we snapshot the value of the global `reward_index` and store it under this user's individual
RewardInfo.index field.

The magic is to realize that for a given user, at any time, if we know:
  - (1) CurrRI = value of current `reward_index`;
  - (2) PrevRI = snapshot of global `reward_index` when this user last bonded/unbonded (which we have, because we
      stored it in user's index field);
  - (3) CurrB = amount this user has bound at the current time;

...then we are able to figure out this user's actual share of released rewards in PRISM tokens since the
snapshot was taken! This share is just: CurrB * (CurrRI - PrevRI). This works because (CurrRI - PrevRI) is the
number of PRISM rewards that should be paid to anyone that happened to have 1 bound yluna unit at the time of
PrevRI and kept it bound until the time of CurrRI.