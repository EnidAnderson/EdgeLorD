# LSP Feature Triage Report for EdgeLorD

This document categorizes potential LSP features for the EdgeLorD server into Easy, Medium, and Hard, based on the provided definitions and an analysis of the existing `src/lsp.rs` codebase.

## Core Boundary Classification Rule

To clarify the scope, features are categorized based on these rules:

**EdgeLorD-only (Easy):** Anything based on
*   `ParsedDocument` + `WorkspaceReport` that EdgeLorD already obtains,
*   client-provided text changes,
*   deterministic formatting/sorting of data,
*   optional shell-out to external commands without requiring new structured output.

**Needs core hooks (Medium/Hard):** Anything requiring
*   stable IDs for symbols/goals across edits (not just spans),
*   semantic token classifications beyond “syntax-ish” heuristics,
*   incremental dependency graph across files,
*   interactive proof session state (holes, tactics, step-by-step evolution).

---

## 1. Easy Features (EdgeLorD-only)

These features can be implemented or completed entirely within the `EdgeLorD/` directory without requiring changes to the core or satellites.

### 1.1. Basic Server Initialization and Capabilities

*   **What it does (user-visible)**: The LSP server initializes upon client connection, advertises its supported functionalities (e.g., text document synchronization, hover, document symbols), and confirms readiness.
*   **Which LSP methods it touches**: `initialize`, `initialized`.
*   **Implementation approach in EdgeLorD**: Largely implemented. The `Backend::initialize` method already sets up `ServerCapabilities` for text document sync (incremental), selection ranges, document symbols, hover, code actions (stub), inlay hints, and diagnostic provision. The `initialized` method logs a message. No further core logic needed for basic operation.

### 1.2. Document Lifecycle Management & In-Memory Text Store

*   **What it does (user-visible)**: The server accurately tracks the content of open documents as clients open, change, save, and close them. It maintains an up-to-date in-memory representation of these documents.
*   **Which LSP methods it touches**: `textDocument/didOpen`, `textDocument/didChange`, `textDocument/didSave`, `textDocument/didClose`.
*   **Implementation approach in EdgeLorD**: Largely implemented. `ServerState.documents` (a `BTreeMap<Url, DocumentState>`) stores `DocumentState` for each URI, containing the document version, `ParsedDocument`, and `WorkspaceReport`. Methods like `upsert_document` and `replace_with_changes` handle updates, while `lsp_changes_to_content_changes` processes incremental content changes from the client.

### 1.3. Deterministic Diagnostics Pipeline (with Stable Tie-Breakers)

*   **What it does (user-visible)**: Ensures that diagnostics (errors, warnings, etc.) are consistently ordered by a predefined canonical sort key before being published to the client. This guarantees stable UI behavior and testability, even across multiple documents or merged diagnostic sources. The publish path ensures per-URI stability and does not interleave diagnostics from different URIs unpredictably.
*   **Which LSP methods it touches**: `textDocument/publishDiagnostics` (implicitly), `textDocument/diagnostic`.
*   **Implementation approach in EdgeLorD**: **Fully implemented for initial sort key, needs URI + full tie-breakers.** The functions `diagnostic_sort_key` and `sort_diagnostics` explicitly define and apply ordering. The full sort key will include: `uri` (string), `range.start.line`, `range.start.character`, `range.end.line`, `range.end.character`, `severity` (with `None` treated consistently), `code` (if present), `source` (if present), `message`.

### 1.4. Basic Hover from Syntax Heuristics

*   **What it does (user-visible)**: Provides rudimentary contextual information or documentation when the user hovers their mouse over specific syntactical elements (e.g., goals, selected spans). This can be polished further.
*   **Which LSP methods it touches**: `textDocument/hover`.
*   **Implementation approach in EdgeLorD**: Partially implemented. The `Backend::hover` method already uses `ParsedDocument::goal_at_offset` to show goal details and `ParsedDocument::selection_chain_for_offset` to display focused span information. This can be extended to include "show current document version / last analysis time" in a small footer and ensure it never panics on a missing parsed document.

### 1.5. Debounce Analysis + Publishing Requests (with Single-Flight)

*   **What it does (user-visible)**: Prevents the LSP server from re-analyzing or re-publishing diagnostics for a document too frequently (e.g., on every single keystroke). It waits for a brief period of user inactivity before processing changes, and critically, ensures that only the *latest* analysis request for a document is processed, canceling or ignoring older, stale requests. This avoids "stale diagnostics flash."
*   **Which LSP methods it touches**: `textDocument/didChange`.
*   **Implementation approach in EdgeLorD**: **To be implemented.** This involves introducing a debouncing mechanism (e.g., using `tokio::time::sleep` and `tokio::sync::mpsc::channel`) within the `did_change` handler. Changes would be accumulated for a short configurable interval. A "single-flight" mechanism (e.g., maintaining a `JoinHandle` or generation counter per URI) will ensure that only the latest pending work proceeds, canceling or ignoring previous analysis tasks.

### 1.6. Deterministic Publish Contract ("No Shadow Stream")

*   **What it does (user-visible)**: Guarantees that diagnostic updates are published only after all processing (including analysis and sorting) is complete and without holding any shared locks that could introduce interleaving or non-determinism. This ensures the client always receives a coherent and stable set of diagnostics.
*   **Which LSP methods it touches**: `textDocument/publishDiagnostics` (implicitly).
*   **Implementation approach in EdgeLorD**: **To be implemented as an invariant.** Ensure diagnostics are fully computed and sorted, and any necessary shared state locks are released *before* initiating the `client.publish_diagnostics` call. The debouncing mechanism will aid in this by creating a clear boundary for when analysis is performed and results are ready to be published.

### 1.7. Basic "Document Symbols" Correctness Checks + Sorting

*   **What it does (user-visible)**: Ensures that the list of document symbols provided to the client is always deterministically sorted (e.g., by range start) and free of duplicates, providing a stable and reliable outline view of the document.
*   **Which LSP methods it touches**: `textDocument/documentSymbol`.
*   **Implementation approach in EdgeLorD**: **To be implemented.** After obtaining symbols from `top_level_symbols(&doc.parsed.text)`, ensure they are sorted deterministically (e.g., by `range.start.line` then `range.start.character`) before being returned as `DocumentSymbolResponse::Nested`. Check for and handle potential duplicates if `top_level_symbols` can produce them.

### 1.8. Trace Parsing Compatibility Helper

*   **What it does (user-visible)**: Provides a utility function within EdgeLorD to parse trace data, gracefully handling both the new `trace-v1` wrapper format and older legacy cons-list traces. Even if not immediately consumed, this ensures future compatibility.
*   **Which LSP methods it touches**: None directly, but will be an internal helper.
*   **Implementation approach in EdgeLorD**: **To be implemented.** Create an `extract_trace_steps(term) -> Option<steps>` helper function that can successfully parse either `(quote (trace-v1 ...))` or `(quote (...))` (legacy cons-list). This helper should include unit tests.

### 1.9. Basic Configuration & Logging Surface

*   **What it does (user-visible)**: Provides a simple way for users to configure basic LSP server behaviors (e.g., `debounceMs`, `logLevel`, an optional `externalCommand` block). Logs internal server operations to aid debugging.
*   **Which LSP methods it touches**: `initialize` (for `initializationOptions`).
*   **Implementation approach in EdgeLorD**: Partially implemented (logging, `debounceMs` via `Config`).
    *   **Configuration**: Extend `Config` to include `logLevel` and an `externalCommand` block (even if the command execution is not yet implemented). Parse `InitializeParams.initializationOptions` to set runtime parameters, with environment variable fallbacks if desired (though `initializationOptions` is preferred for LSP context).
    *   **Logging**: Leverage the existing `self.client.log_message` for LSP-specific logging, and potentially an internal logging framework (like `tracing` or `log`) for more detailed server diagnostics, respecting `logLevel`.

### 1.10. Minimal Code Actions (Purely Textual)

*   **What it does (user-visible)**: Offers simple, purely textual code transformations as LSP code actions. These are "safe" in that they only manipulate text without requiring semantic analysis of the code structure. Examples: wrapping selection in a `do {}` block, `(quote ...)` wrapping, or converting syntactic sugar.
*   **Which LSP methods it touches**: `textDocument/codeAction`.
*   **Implementation approach in EdgeLorD**: **To be implemented.** Implement `Backend::code_action` to return a list of `CodeActionOrCommand` based on the selected text range. The actions should only involve text manipulation. Ensure deterministic ordering and titles for the code actions.

### 1.11. Configurable External Command Integration (Best-Effort Diagnostics)

*   **What it does (user-visible)**: Allows the LSP server to run a user-configured external command (e.g., `comrade check`, `cargo run -- <args>`). On save or on demand, it executes this command, captures its stdout/stderr, and parses the output. If the output contains stable tags like `trace/...:` or `overlap/...:`, it maps them to structured diagnostics. Otherwise, it emits a single "whole-file" diagnostic at `(0,0)` to surface any unparseable output. This provides immediate feedback without requiring core changes.
*   **Which LSP methods it touches**: Potentially `textDocument/didSave`, or a custom LSP command. `initialize` for configuration.
*   **Implementation approach in EdgeLorD**: **To be implemented.** Use `tokio::process::Command` to execute the configured command with a timeout. Implement parsing logic to extract structured diagnostics (if tags are present) or create a fallback "whole-file" diagnostic. This is the "v0" implementation, not relying on structured output from core.

## 2. Medium Features (Needs Core Hooks)

These features can be implemented mostly in EdgeLorD, but would be significantly better with optional core hooks or more exposed internal APIs.

### 2.1. Richer Spans / Multi-Span Diagnostics (Primary + Notes)

*   **What EdgeLorD can do now**: Converts `source_span::Span` from `WorkspaceDiagnostic` into LSP `Range` objects, providing diagnostics with specific character-level ranges. This covers the "exact ranges" aspect.
*   **What would be improved with core hooks**: If `new_surface_syntax` (the core component providing `WorkspaceReport`) could expose richer, more granular span types for different semantic constructs (e.g., distinguishing the span of a faulty identifier vs. an entire expression, or the precise span of an operator causing an error), and potentially primary + related information spans (e.g., "error here, but defined here"), EdgeLorD could convey even more accurate and targeted diagnostic locations.
*   **Whether Trace v1 / scope_digest helps**: Yes, the underlying determinism and stability guaranteed by Trace v1 and scope digests are crucial for ensuring that these exact spans are consistently generated and associated with the correct code elements, even across incremental changes.

### 2.2. Semantic Tokens

*   **What EdgeLorD can do now**: The `ParsedDocument` (from `crate::document`) has some understanding of document structure through methods like `goal_at_offset`, `selection_chain_for_offset`, and `top_level_symbols`. This implies a form of parsing and tokenization already exists.
*   **What would be improved with core hooks**: A core API (e.g., an extension to `ParsedDocument` or a new service in `new_surface_syntax`) that explicitly exposes a stream or map of semantic tokens (identifiers, keywords, types, literals, comments, etc.) along with their precise spans and semantic classifications would be ideal. This would enable EdgeLorD to implement the `textDocument/semanticTokens` LSP method comprehensively.
*   **Whether Trace v1 / scope_digest helps**: Indirectly. The consistent parsing and semantic analysis ensured by Trace v1 and stable scope digests are foundational for reliably identifying and categorizing semantic tokens.

### 2.3. Incremental Recompilation / Analysis

*   **What EdgeLorD can do now**: The `Backend::did_change` method already passes incremental `TextDocumentContentChangeEvent`s to `ComradeWorkspace::did_change`. This means the client-side of incremental updates is handled, and `ComradeWorkspace` is informed of changes.
*   **What would be improved with core hooks**: While `ComradeWorkspace` handles *some* incremental changes, a dedicated "compiler service" within the core, offering fine-grained invalidation and re-evaluation APIs, would significantly improve performance. This would allow EdgeLorD to query for updated analysis results only for the affected parts of the code, rather than potentially re-analyzing larger sections of the document or workspace. Examples include hooks to update a specific AST node or re-type-check a single function.
*   **Whether Trace v1 / scope_digest helps**: Absolutely. Trace replay and stable scope digests are essential for verifying that incremental recompilation yields the same correct and deterministic results as a full rebuild, preserving the integrity of the analysis.

### 2.4. Code Actions for Safe Refactors (Semantic)

*   **What EdgeLorD can do now**: The `Backend::code_action` method is currently a stub, but will implement purely textual code actions.
*   **What would be improved with core hooks**: Core APIs that provide a robust Abstract Syntax Tree (AST) representation of the code, along with methods to query and apply predefined, semantics-preserving transformations (refactorings) to this AST. This would enable EdgeLorD to implement powerful "extract function," "rename symbol," "introduce variable," or "apply suggestion" code actions reliably. The core would validate the safety and correctness of these refactoring operations.
*   **Whether Trace v1 / scope_digest helps**: Yes. The ability to guarantee deterministic behavior and strong semantic closure (as provided by Trace v1 and scope digests) is paramount for ensuring that refactoring code actions are genuinely "safe" and do not subtly alter the program's meaning or introduce bugs.

### 2.5. Core-Enhanced External Command Integration (JSON Diagnostics + Byte Spans)

*   **What EdgeLorD can do now**: Basic external command execution with best-effort, heuristic-based parsing for diagnostics (as described in Easy Feature 1.11).
*   **What would be improved with core hooks**: If the core (or external tools) could provide a structured output format for diagnostics (e.g., JSON) including file paths and precise byte spans, EdgeLorD could accurately map these to LSP `Range` objects for much better user experience and robust error reporting. This is the "v1" implementation, relying on structured output from core.
*   **Whether Trace v1 / scope_digest helps**: Not directly for the parsing itself, but if the external command *is* a core tool, then trace/digest ensures the output of that tool is deterministic and consistent, which is crucial for reliable diagnostics.

## 3. Hard Features (Requires New Core APIs / Semantics)

These features require new core APIs or fundamental changes to the semantics of the core system.

### 3.1. Proof-State / Hole Interaction with Trace Replay

*   **Why it needs core work**: This feature goes beyond static analysis, requiring dynamic interaction with the core's theorem prover or proof engine. It demands specific APIs to inspect intermediate proof states, identify unproven "holes" (subgoals) within a proof, and directly interface with the system that manages certified artifacts and trace replay for verification. This is fundamentally about interactive proof development.
*   **What core change would unlock it**: New core APIs that expose the internal state of ongoing proofs, allow enumeration and inspection of proof "holes" or obligations, and provide mechanisms to query the status and metadata of certified artifacts derived from successful trace replay. This might involve a dedicated "proof session" API.

### 3.2. Tactic-like Interactive Steps

*   **Why it needs core work**: This implies a real-time, bidirectional protocol where the LSP server can send high-level "tactics" or commands to the core's proof engine (similar to interactive theorem provers like Coq or Lean) and receive immediate, structured feedback on their application and effect on the proof state. This is a complex form of interactive computation with the core.
*   **What core change would unlock it**: A new core "interactive tactic API" or "proof object streaming service" that allows the LSP to send structured commands (tactics) and, in response, receive a stream of updated proof objects, partial proof trees, or refined subgoals, enabling a truly interactive proof development experience within the editor.

### 3.3. Cross-File Dependency Graph & Precise Invalidation

*   **Why it needs core work**: Building and maintaining a truly precise and efficient cross-file dependency graph for the entire workspace, complete with accurate invalidation logic, requires deep access to the compiler's internals. It cannot be reliably derived from external tool output or simple string matching. The compiler is the sole authority on file dependencies and how changes in one file impact others.
*   **What core change would unlock it**: A core API that exposes the compiler's internal understanding of the project's module graph or file dependencies. This API would allow the LSP to query which files depend on others, receive notifications for specific file invalidations, and efficiently determine the minimal set of files that need re-analysis after a change.

---
End of Feature Triage Report
