# Prism Limit Order

This contract provides limit order functionality for astroport trading pairs. Users submit "orders" containing an offer (sell) asset and an ask (buy) asset where they require a certain ask asset amount in return for their provided offer asset amount. Executions occur through external users/bots submitting ExecuteOrder messages on these limit orders when conditions are favorable. The user executing the order is rewarded with a percentage of the fee and a portion of any excess ask asset amount captured from the swap. The protocol also captures a fee which is sent to the [gov](/contracts/prism-gov) contract to reward PRISM stakers. Note that PRISM can be used as an intermediate swap pair in the event that the specified offer/ask trading pair is not directly available.  

## ExecuteMsg:
  - **AddPair**: Add a trading pair to the supported pairs list.  Must be called by contract owner.
  - **UpdateConfig**: Update configuration parameters including fee collector, order fee, min fee value, and executor fee portion.  Must be called by contract owner. 
  - **SubmitOrder**: Submit an order containing an offer asset and an ask asset.  For native offer assets, the offer quantity must be sent with the message. For token offer assets, the user must have previously increased the allowance for this contract to transfer itself the offer amount.  
  - **CancelOrder**: Cancel the specified order.
  - **ExecuteOrder**: Execute the specified order.  If the swap is unable to return the required asset amount (taking into account protocol fees), then this operation will fail. 

## QueryMsg:
  - **Config**: Retrives contract configuration paraameters. 
  - **Order**: Retrieve order information for the specified order id.
  - **Orders**: Retrieve all orders submitted from the specified address.
  - **LastOrderId**: Retrieve the order id corresponding to the last order submitted.  
