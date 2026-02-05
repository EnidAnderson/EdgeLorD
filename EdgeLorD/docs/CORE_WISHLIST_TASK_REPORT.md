I have completed the core wishlist task.

**Summary of changes:**
*   **Helper Function**: Added `fn trace_err(tag: &str, msg: impl Display) -> String` to `../clean_kernel/tcb_core/src/prelude.rs`.
*   **Error Site Rewriting**: Modified all `return Err(...)` sites within the `replay_trace` function in `../clean_kernel/tcb_core/src/prelude.rs` to use the `trace_err` helper with the specified canonical tags.
    *   This includes `trace/unsupported-version`, `trace/malformed`, `trace/before-hash-mismatch`, `trace/rule-ix-oob`, `trace/rule-fp-mismatch`, `trace/step-failed`, and `trace/after-hash-mismatch`.
*   **Unit Tests**: Added new unit tests to `../clean_kernel/tcb_core/src/tests/unit_tests/trace_replay.rs` to specifically trigger and verify each of the canonical error tags. These tests assert only the tag prefix, as required.

**Verification Status:**
I attempted to run `cargo test -p tcb_core` to verify these changes, but the command failed with the following error:
`error: could not create home directory: '/Volumes/EnidsAssets/rust/.rustup': Permission denied (os error 13)`

This indicates a permission issue with the Rust environment setup on your system, which prevents `cargo test` from running. I am unable to resolve this environmental configuration problem directly.

The code changes strictly adhere to the provided specification, and I have followed all instructions regarding invariants and canonical tags. Once the permission issue is resolved, running `cargo test -p tcb_core` should confirm the correctness of these modifications.
