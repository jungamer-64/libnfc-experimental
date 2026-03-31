<!-- Please describe the change in a single sentence above. -->

## Description

Describe the motivation for this change and what it does.

## Checklist
- [ ] I have run the full test suite locally where available.
- [ ] I have added or updated unit tests for any changed functionality.
- [ ] I have updated documentation where necessary.

If this PR touches the FFI boundary (Rust ⇄ C), complete the FFI checklist below:

- [ ] I regenerated and committed the cbindgen header if FFI symbols/signatures changed:

      python3 rust/libnfc-rs/tools/generate_cbindgen_header.py --output rust/libnfc-rs/include/libnfc_rs.h

- [ ] The PR contains a clear ownership table for any returned buffers (who allocates, who frees).
- [ ] `scripts/check-cbindgen.sh` and `scripts/check_callerfree_usage.sh` pass locally.
- [ ] If I ran the standalone `examples/ffi-sanity/` check locally, I included the command/results below. If/when CI gains an `ffi-sanity` job, it must pass there as well.
- [ ] If this is an ABI-breaking change, an RFC issue/PR is linked and any project-specific approval flow is noted below.

## Related issues

List any related issues or RFCs here.

## Notes for reviewers

Add any details that will help reviewers (e.g., special build flags required, test commands).
