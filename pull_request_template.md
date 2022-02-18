## Summary
*(Add a brief explanation of this change)*

## Checklist
*(For both PR authors and reviewers)*

**Fund transfer/allocation**:
- [ ] Does this change involve any interaction with stored vault balances, fund transfers, or reward allocations?
- [ ] If so, have you verified that you're not inadvertently transferring/allocating stored vault balances?  See [contract_balances.md][1] to verify contract existing balance storage and allocations.

**Endpoint permissions**:
- [ ] Have you verified permissions for all endpoints applicable to this change? Permissions should be as restrictive as possible and only allow what's truly needed.
- [ ] Are there any endpoints that used to be public and now should be made private? This applies to this contract as well as other contracts (for example, you stopped calling an endpoint in a different contract, so that endpoint can now be locked down).

**Testing**:
- [ ] Have you written test coverage for all code paths (excluding logic error code paths) and verified coverage using tarpaulin?

**Documentation**:
- [ ] Have you updated documentation (contract's README.md, [contract_balances.md][1], and inlined rust comments)?


[1]: https://github.com/prism-finance/prism-contracts/blob/main/contract_balances.md
