# EdgeLorD Integration Notes

- The Comrade workspace now dominates CLI and daemon hooks: `ComradeWorkspace::did_open`/`did_change` compile, cache, and expose diagnostics/fingerprints, so reuse its reports rather than re-implementing parsing.
- The CLI helpers (`satellites::surface_maclane::comrade_command`) now read diagnostics and bundles directly from the workspace report, so any downstream service should call those methods to stay synchronized with the canonical surface semantics.
- The workspace tests (`new_surface_syntax::tests::workspace`) demonstrate diagnostics/fingerprint parity with the CLI; run them to see the concrete expectation.
- `WorkspaceReport` diagnostics now carry spans, severity, and optional phase codes, so the LSP can render precise ranges instead of whole-document fallbacks.
- If EdgeLorD wants to share this pipeline, keep a single workspace instance (`ComradeWorkspace`) and treat document URIs as `DocumentKey`s (e.g., `uri.to_string()`). Call `did_open` after storing text, `did_change` or `set_document_text` after edits, and `did_close` when a document closes.
- Updating the LSP server this way keeps the long-lived workspace cache, deterministic compile/fingerprint invariants, and CLI/LSP semantics in harmony.

## Diagnostics and deterministic ordering

- Consume the `WorkspaceReport` for every lifecycle event (`didOpen`, `didChange`, `didClose`). Do **not** publish diagnostics computed from any other cache or parsing pipeline; the report is the canonical error stream.
- Merge goal/parse overlays on top of the workspace diagnostics only after sorting by `(severity, span.start, span.end, message)` so the published list is deterministic. Keep the workspace lock held only long enough to mutate the workspace; publish diagnostics after the lock is released.
- Avoid padding diagnostics with fingerprints—fingerprint/state data belongs in `window/logMessage` or server state, not in the diagnostic channel.

## Tests and verification

- Run `cargo test -p satellites --doc` to hit the compile-fail guard and workspace tests that back up the CLI path.
- Run `cargo test -p satellites canonical_alignment` and `cargo test -p satellites canonical_domains` (these live in `satellites/tests/`) to ensure the backend fingerprint alignment and domain tags are ratcheted.
- Run `cargo test -p EdgeLorD` to exercise the LSP harness against the updated workspace hooks.
- These commands correspond to the adjacent "main checklist" guardrails: canonical alignment, domain separation, and workspace-report-based diagnostics. Keeping them green means you can continue building the pattern-graph backend without slipping back to legacy paths.
