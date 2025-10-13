<!-- Please describe the change in a single sentence above. -->

## Description

Describe the motivation for this change and what it does.

## Checklist
- [ ] I have run the full test suite locally where available.
- [ ] I have added or updated unit tests for any changed functionality.
- [ ] I have updated documentation where necessary.

If this PR touches the FFI boundary (Rust ⇄ C), complete the FFI checklist below:

- [ ] I regenerated and committed the cbindgen header if FFI symbols/signatures changed:

      cbindgen --config rust/libnfc-rs/cbindgen.toml --crate libnfc-rs --output rust/libnfc-rs/include/libnfc_rs.h

- [ ] The PR contains a clear ownership table for any returned buffers (who allocates, who frees).
- [ ] `scripts/check-cbindgen.sh` and `scripts/check_callerfree_usage.sh` pass locally.
- [ ] `ffi-sanity` integration tests pass and are included in CI.
- [ ] If this is an ABI-breaking change, an RFC issue/PR is linked and approved by the FFI Maintainer.

## Related issues

List any related issues or RFCs here.

## Notes for reviewers

Add any details that will help reviewers (e.g., special build flags required, test commands).
