# Summary

- Log handler group and request variant name before delegation
  in all grouped `node_*` handlers and legacy flat methods
- All 19 existing `roxyd` tests pass; no business-logic
  changes

Closes <https://github.com/aicers/roxy/issues/596>

## Test plan

- [x] `cargo test --bin roxyd` - 19 tests pass
- [x] `cargo clippy --tests --all-features --bin roxyd` - no warnings
