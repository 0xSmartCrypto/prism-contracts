## Summary
*(Add a brief explanation of this change)*

## Checklist
*(For both PR authors and reviewers)*

For any endpoint you are adding or modifying:
- Should the endpoint be public?
  - [ ] Add a comment in the method explaining why should anyone on the public internet be able to call it.
- Should the endpoint be private?
  - [ ] Write a unit test that makes sures `unauthorized` errors are being returned for unauthorized callers.
  - [ ] Remove any comments mentioning that the endpoint is public (perhaps it used to be public before your PR).
