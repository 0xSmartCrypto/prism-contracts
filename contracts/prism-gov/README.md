# Prism Governance

This contract provides standard governance polling/voting functionality as well as support for minting/redeeming xPRISM and staking xPRISM for voting rights. The redemption process for xPRISM is subject to a 21-day holding period. Only staked xPRISM holders are allowed to vote, and each staked xPRISM token allows for one vote. The PRISM/xPRISM relationship is similar to UST/aUST, where the xPRISM is constantly appreciating in value with respect to PRISM. This appreciation is due to the protocol fees from the yasset-staking contract being converted to PRISM (inside the collector contract) and then sent to us, which is then included in our PRISM/xPRISM exchange rate calculation. Polling and voting logic works similar to the anchor protocol: https://docs.anchorprotocol.com/protocol/anchor-governance.

## ExecuteMsg:
  - **MintXprism** (Cw20 receive hook from PRISM contract): Mint xPRISM by supplying PRISM.
  - **RedeemXprism** (Cw20 receive hook from xPRISM contract): Redeem PRISM by supplying xPRISM.  
    There is a 21 day hold on the redeemed PRISM tokens before they are allowed to be claimed.  
  - **ClaimRedeemedXprism**: Claim any redeemed PRISM tokens that have previously been redeemed with a call to RedeemXprism.   
  - **StakeVotingTokens** (Cw20 receive hook from xPRISM contract): Stake xPRISM in order to receive voting rights.
  - **WithdrawVotingTokens**: Withdraw voting tokens.  The max number of tokens that are currently being used to vote on any live poll are locked and cannot be withdrawn.  
  - **PostInitialize**: This must be called after initialize in order to set the xprism_token config parameter.  Can only be called once, and must be called by contract owner.
  - **UpdateConfig**: Updates config parameters, must be called by contract owner.  
  - **CreatePoll** (Cw20 receive hook from xPRISM contract): Create a poll to be voted on by staked xPRISM holders.  Requires an initial xPRISM deposit greater than the configuration proposal_deposit amount.  This deposit is returned if a quorom is reached.  Poll consists of a title, description, link, and a smart contract message to execute in the event that the poll passes.  
  - **CastVote**: Cast a vote on the specified poll_id with the specified amount of voting tokens.  Users can only vote once on any given poll and their voting amount must be less than or equal to their balance of voting tokens.  
  - **EndPoll**: Once the voting period has expired, anyone can call EndPoll in order to finalize the poll.  At this time, if no quorom was reached, the poll is rejected and the initial deposit from the CreatePoll message is kept in the governance PRISM contract?  This should be burned I believe?
    The poll passes if the number of yes votes meets the required threshold, otherwise it fails.   
  - **ExecutePoll**: If a poll passes, any user can call this message which will execute the message associated with the poll.  
  - **SnapshotPoll**: This message is used to take a snapshot of the xprism token supply which is used for quorum calculation.  
  
## QueryMsg:
  - **Config**: Retrieve contract configuration.
  - **VotingTokens**: Returns total voting tokens (locked xPRISM) and a list containing locked balance for each in progress poll.
  - **Poll**: Retrieve poll information for the specified poll id.
  - **Polls**: Return poll information for every poll has been created. Provides support for pagination.  
  - **Voter**: Queries a voters response (yes/no and vote amount) for the specified voter address and poll id. 
  - **Voters**: Queries all voters responses for the specified poll id.  Provides support for pagination.  
  - **PrismWithdrawOrders**: Queries the pending xPRISM redeems, which are subject to a 21 day holding period.  
