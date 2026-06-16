<!-- Please describe the change in a single sentence above. -->

## Description

Describe the motivation for this change and what it does.

## Checklist
- [ ] I have run the full test suite locally where available.
- [ ] I have added or updated unit tests for any changed functionality.
- [ ] I have updated documentation where necessary.

If this PR touches the Rust/C FFI boundary, complete the FFI checklist below:

- [ ] The PR contains a clear ownership table for any returned buffers (who allocates, who frees).
- [ ] `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passes locally.
- [ ] `cargo check --manifest-path rust/Cargo.toml --workspace` passes locally.
- [ ] `cargo clippy --manifest-path rust/Cargo.toml --workspace -- -D warnings` passes locally.
- [ ] `scripts/check_callerfree_usage.sh` and `scripts/check-no-retired-private-header-includes.sh` pass locally.
- [ ] If I ran C ABI smoke tests locally, I included the command/results below (for example `ctest --test-dir build -R 'ffi_sanity|public_compat_smoke' --output-on-failure`).
- [ ] If this is an ABI-breaking change, an RFC issue/PR is linked and any project-specific approval flow is noted below.

## Related issues

List any related issues or RFCs here.

## Notes for reviewers

Add any details that will help reviewers, such as special build flags or test commands.
