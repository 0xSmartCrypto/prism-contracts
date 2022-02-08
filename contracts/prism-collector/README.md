# Prism Collector

This contract is responsible for collecting protocol fees, converting those assets to PRISM, and sending the resulting PRISM tokens to the [gov](/contracts/prism-gov) contract, which results in xPRISM accruing value.  

Sources of protocol fees include:
- all delegator rewards from unstaked y-assets (denominated in pluna/yluna)
- 10% of delegator from staked y-assets (denominated in pluna/yluna)
- all airdrop rewards from unstaked y-assets (denominated in airdrop token)
- 10% of airdrop rewards from staked y-assets (denominated in airdrop token)
- all prismswap protocol fees (denominated in the ask asset from every swap)

## ExecuteMsg:
- **ConvertAndSend**: Convert the input assets into PRISM and send the resulting PRISM to the specified receiver.
- **Distribute**: Convert our current balance of the specified assets into PRISM and sends the resulting PRISM to the [gov](/contracts/prism-gov) contract.  This method is executed at random intervals by an automated bot.  
- **BaseSwapHook**: Hook when we need an intermediate swap to UST, this method converts our entire UST balance to PRISM and sends to the configured receiver.  

## QueryMsg:
- **Config**: Retrieves configuration information for this contract.
