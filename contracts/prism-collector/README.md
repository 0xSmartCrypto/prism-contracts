# Prism Collector

This contract is responsible for collecting protocol fee rewards and unstaked y-asset delegator rewards from the yasset-staking contract, converting those rewards to PRISM, and sending those PRISM tokens to the [gov](/contracts/prism-gov) contract. Additionally, when the yasset staker has elected to stake with xprism staking mode, this contract provides functionality to swap input assets to PRISM and send them back to the specified receiver.

## ExecuteMsg:
- **ConvertAndSend**: Convert the input assets into PRISM (via astroport) and send the resulting PRISM to the specified receiver.  This contains logic to perform an intermediate swap to UST if there is no direct pair from input asset to PRISM.  This method is called by the [yasset-staking](/contracts/prism-yasset-staking) when claiming rewards but only if the staker is staking with xprism mode. 
- **Distribute**: Convert our current balance of the input tokens into PRISM (via astroport) and send the resulting PRISM to the [gov](/contracts/prism-gov) contract.  This method also contains logic to perform an intermediate swap to UST if there is no direct pair from input token to PRISM.  This method is called by ???.  
- **BaseSwapHook**: Hook when we need an intermediate swap to UST, this method converts our UST balance to PRISM and sends to the configured receiver.  The receiver will either be the yasset staker or the governance contract, depending on whether this was called from ConvertAndSend or Distribute.  

## QueryMsg:
- **Config**: Retrieves configuration information for this contract.
