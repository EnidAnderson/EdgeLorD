# RAG Semantic Bundle (repo-safe)

- Repo root: `/Users/e/Documents/MotivicCohomology/GitLocal/EdgeLorD`
- Git HEAD: `6ecfa20952749bb57cb096dacccd1eabbb4cdec9` (dirty)
- Generated (UTC): `2026-02-08T23:03:43.912951+00:00`
- Mode: `changed` (base `origin/main`)

## Audit
- considered (git paths): **90**
- included: **79**
- skipped: not-allowed=12, excluded=0, missing=0, too-large=0
- budgets: max_files=3000, max_file_bytes=2000000, max_output_kb=6000

## Cargo overview

### `Cargo.toml`
- package: **edgelord-lsp** 0.1.0
- deps (15): tokio, tower-lsp, serde, serde_json, thiserror, source_span, codeswitch, comrade_lisp, sniper_db, async-trait, crc32fast, tantivy, tcb_core, sha2, hex

## Rust semantics (lossy)

- Included Rust files: **37**
- include_private=0; brief=1

### `src/caching.rs`
- items:
  - (pub trait) `pub trait SnapshotStore: Send + Sync` — Minimal snapshot storage interface (L1/L2 abstraction) L1 (InMemoryStore): Fast in-process cache (BTreeMap LRU) L2 (SniperDbStore): Persistent backing (survives restarts) ModuleSnapshotCache checks L1 first, then L2, then compiles.
  - (pub struct) `pub struct SnapshotStoreKey(pub HashValue);` — Key for snapshot storage: derive from 4-component cache key
  - (pub fn) `pub fn from_cache_key(` — Create from the 4-component cache key
  - (pub struct) `pub struct SerializedSnapshot` — Serialized snapshot for storage (compact form, Phase 1.2B: deferred serialization)
  - (pub fn) `pub fn from_module_snapshot(snapshot: &ModuleSnapshot) -> Self`
  - (pub fn) `pub fn to_module_snapshot(`
  - (pub struct) `pub struct InMemorySnapshotStore` — L1 (in-memory) snapshot store
  - (pub fn) `pub fn new() -> Self`
  - (pub struct) `pub struct SniperDbSnapshotStore` — L2 (persistent) snapshot store backed by SniperDatabase
  - (pub fn) `pub fn new(db: Arc<SniperDatabase>) -> Self`
  - (pub enum) `pub enum Phase1MissReason` — Phase 1 (Module Snapshot) cache miss reasons
  - (pub fn) `pub fn to_outcome_string(&self) -> &'static str` — Convert to outcome string for CSV
  - (pub enum) `pub enum Phase1_1MissReason` — Phase 1.1 (Workspace-Aware) cache miss reasons
  - (pub fn) `pub fn to_outcome_string(&self) -> &'static str` — Convert to outcome string for CSV
  - (pub enum) `pub enum CacheOutcome<M>` — Cache outcome: either a hit or a miss with reason
  - (pub fn) `pub fn to_outcome_string(&self) -> String` — Convert to outcome string for CSV
  - (pub trait) `pub trait CacheOutcomeString` — Trait for types that can be converted to outcome strings
  - (pub enum) `pub enum CacheGetResult<V>` — C2.4: Cache lookup result - refactor-safe outcome classification Instead of counter-based or log-based hit detection, the cache lookup directly returns whether it was a hit or miss with a classified reason. This makes outcome determination part of the control flow and immune to refactoring.
  - (pub enum) `pub enum CacheGetResult1_1<V>`
  - (pub fn) `pub fn to_outcome_string(&self) -> String`
  - (pub fn) `pub fn is_hit(&self) -> bool`
  - (pub fn) `pub fn to_outcome_string(&self) -> String`
  - (pub fn) `pub fn is_hit(&self) -> bool`
  - (pub struct) `pub struct CacheKeyBuilder` — Builder for CacheKey with safe defaults and validation.
  - (pub fn) `pub fn new() -> Self`
  - (pub fn) `pub fn options(mut self, fp: HashValue) -> Self`
  - (pub fn) `pub fn workspace_snapshot(mut self, hash: HashValue) -> Self`
  - (pub fn) `pub fn unit_id(mut self, id: impl Into<String>) -> Self`
  - (pub fn) `pub fn unit_content(mut self, hash: HashValue) -> Self`
  - (pub fn) `pub fn dependencies(mut self, fp: HashValue) -> Self`
  - (pub fn) `pub fn build(self) -> Result<CacheKey, String>`
  - (pub struct) `pub struct CacheKey` — Canonical cache key combining all inputs that affect compilation. As per PHASE_1_1_ACCEPTANCE_SPEC.md §1.1: - OptionsFingerprint: hash of canonical serialization of compile options (BTreeMap/sorted) - WorkspaceSnapshotHash: hash of workspace state (open docs + dependencies) - UnitId: canonicalized file identifier (prefer FileId, normalize Url if needed) - UnitContentHash: hash of file content at compilation time - DependencyFingerprint: hash of transitive dependencies INV D-CACHE-1 precondition: "same options" means bit-for-bit identical canonical bytes
  - (pub struct) `pub struct CacheValue` — Cached compilation output and associated metadata.
  - (pub struct) `pub struct ModuleSnapshot` — Phase 1: Module snapshot backed by SniperDB A module snapshot captures the compilation result for a single file with a COMPLETE key that includes all inputs affecting compilation: - file_id: Deterministic file identifier - content_hash: Hash of file content - options_fingerprint: Hash of compile options - dependency_fingerprint: Hash of transitive dependencies INV PHASE-1-MODULE-1 (Sound Reuse): Only reuse when ALL inputs match INV PHASE-1-MODULE-2 (Complete Key): Must include options, dependencies, content INV PHASE-1-MODULE-3 (Content Stability): Same inputs → same snapshot, always
  - (pub struct) `pub struct ModuleSnapshotCache` — Phase 1.2A: Module snapshot cache with L1/L2 storage Implements a 2-level cache: - L1: In-memory BTreeMap LRU (fast, hot) - L2: SniperDB persistent store (survives restarts, auditable) Tracks snapshots by a COMPLETE 4-component key: (file_id, content_hash, options_fingerprint, dependency_fingerprint) This ensures sound reuse: only reuse when ALL compilation inputs match. Prevents silent reuse of stale data when dependencies or options change. Strategy (L1/L2): 1. Check L1 in-memory cache (fast path) 2. Check L2 SniperDB persistent store (cross-session reuse) 3. Cache miss: return None, compute fresh 4. Write-through to both L1 and L2 on insert
  - (pub struct) `pub struct ModuleSnapshotStats` — Statistics for Phase 1 module snapshot cache
  - (pub fn) `pub fn hit_rate(&self) -> f64`
  - (pub fn) `pub fn record_hit(&mut self)`
  - (pub fn) `pub fn record_miss(&mut self)`
  - (pub fn) `pub fn new(db: Arc<SniperDatabase>) -> Self` — Create a new module snapshot cache with L1 (in-memory) and L2 (SniperDB) backing.
  - (pub fn) `pub fn with_max_entries(db: Arc<SniperDatabase>, max_entries: usize) -> Self` — Create with custom max entries.
  - (pub fn) `pub fn get(` — Retrieve module snapshot with a COMPLETE 4-component key (L1/L2 lookup). INV PHASE-1-MODULE-1: Only reuse when all inputs match - file_id: Source file identifier - content_hash: File content hash - options_fingerprint: Compile options hash - dependency_fingerprint: Transitive dependencies hash Phase 1.2A: L1/L2 lookup strategy 1. Check L1 (in-memory BTreeMap) - fast, hot 2. Check L2 (SniperDB persistent store) - cross-session reuse 3. Miss: return None, compile fresh 4. L2 hit: promote to L1 for next access Returns None on cache miss (any component mismatch = miss).
  - (pub fn) `pub fn insert(&mut self, snapshot: ModuleSnapshot)` — Insert a module snapshot into the cache with complete key (write-through L1/L2). Snapshot must include all input components (content_hash, options_fingerprint, dependency_fingerprint) to ensure sound reuse. Phase 1.2A: Write-through strategy 1. Insert into L1 (in-memory BTreeMap) 2. Insert into L2 (SniperDB persistent store) 3. Evict oldest L1 entries if exceeding max_entries This ensures cross-session reuse and makes caching auditable.
  - (pub fn) `pub fn stats(&self) -> ModuleSnapshotStats` — Get current statistics.
  - (pub fn) `pub fn clear(&mut self)` — Clear all cached snapshots (L1 only; L2 persists).
  - (pub fn) `pub fn len(&self) -> usize` — Get L1 (in-memory) cache size.
  - (pub fn) `pub fn is_empty(&self) -> bool` — Check if L1 cache is empty.
  - (pub fn) `pub fn reset_stats(&mut self)` — Reset statistics (for testing).
  - (pub struct) `pub struct CacheStats` — Statistics for cache hits/misses and performance analysis.
  - (pub fn) `pub fn hit_rate(&self) -> f64`
  - (pub fn) `pub fn total_operations(&self) -> u64`
  - (pub fn) `pub fn record_miss(&mut self, reason: impl Into<String>)`
  - (pub fn) `pub fn record_hit(&mut self)`
  - (pub trait) `pub trait CacheStore: Send + Sync` — CacheStore trait: abstraction over storage backends (memory, SniperDB, etc.) Phase 1.2 + Phase 2A: Support pluggable backends while keeping correctness invariants. All backends must preserve: - INV D-CACHE-2 (Sound reuse): identical key → identical cached value - Single-flight semantics: operations atomic with respect to compilation gate
  - (pub struct) `pub struct InMemoryCacheStore` — InMemoryCacheStore: Thread-safe in-memory cache implementation. Invariants: - INV D-CACHE-1 (Purity): Same key → identical output, always - INV D-CACHE-2 (Sound reuse): Reuse only when key exactly matches - INV D-CACHE-3 (Monotone invalidation): Edits invalidate affected caches Thread-safety: Internally uses Arc<RwLock<>> to allow safe concurrent access. Cache must be accessed inside single-flight gate to preserve no-stale-diagnostics.
  - (pub type) `pub type ModuleCache = InMemoryCacheStore;` — Backward-compatible alias for existing code
  - (pub fn) `pub fn new() -> Self` — Create a new InMemoryCacheStore with default settings.
  - (pub fn) `pub fn with_max_entries(max_entries: usize) -> Self` — Create a new InMemoryCacheStore with custom max entries.
  - (pub fn) `pub fn get(&self, key: &CacheKey) -> Option<CacheValue>` — Attempt to retrieve a cached value for the given key. Returns None if not found (cache miss). Updates statistics automatically.
  - (pub fn) `pub fn insert(&self, key: CacheKey, value: CacheValue)` — Insert a compiled value into the cache. If max_entries is reached, oldest entries are evicted (LRU-style).
  - (pub fn) `pub fn clear(&self)` — Clear all cached entries.
  - (pub fn) `pub fn len(&self) -> usize` — Return the number of cached entries.
  - (pub fn) `pub fn is_empty(&self) -> bool` — Check if cache is empty.
  - (pub fn) `pub fn stats(&self) -> CacheStats` — Get current statistics (clone).
  - (pub fn) `pub fn stats_mut(&self) -> CacheStats` — Get mutable access for statistics (for tests).
  - (pub fn) `pub fn reset_stats(&self)` — Reset statistics (for benchmarking between test runs).

### `src/db_memo.rs`
- items:
  - (pub struct) `pub struct DbMemo` — Wrapper for SniperDB memo operations
  - (pub fn) `pub fn new(db: Arc<SniperDatabase>) -> Self` — Create a new DB memo wrapper
  - (pub fn) `pub async fn memo_get_or_compute<F, Fut>(` — Retrieve a memoized compilation result, or compute it if not found. Hard invariant: If input_digest is found in memo, the cached output is returned. Otherwise, the compute function is called exactly once and the result is memoized. This implements "single-flight" semantics: concurrent requests for the same input will coordinate through the database (SniperDB's internal locking).
  - (pub fn) `pub fn memo_put(` — Pre-compute and store a memoization result (for testing/benchmarking)
  - (pub fn) `pub fn memo_contains(&self, input: &CompileInputV1) -> bool` — Check if a result is memoized (for testing)
  - (pub fn) `pub fn db(&self) -> &Arc<SniperDatabase>` — Retrieve the SniperDatabase reference (for testing/debugging)

### `src/highlight.rs`
- items:
  - (pub struct) `pub struct HighlightCtx<'a>`
  - (pub enum) `pub enum SymbolRole`
  - (pub fn) `pub fn to_lsp_type(&self) -> SemanticTokenType`
  - (pub fn) `pub fn modifiers(&self) -> u32`
  - (pub const) `pub const LEGEND_TOKEN_TYPES: &[SemanticTokenType] = &[`
  - (pub const) `pub const LEGEND_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[`
  - (pub fn) `pub fn tokens_to_lsp_data(text: &str, tokens: &mut [(ByteSpan, SymbolRole)]) -> Vec<SemanticToken>` — Encodes internal tokens to LSP delta format (Absolute -> Relative). Must be sorted by Position.
  - (pub fn) `pub fn compute_layer0_structural(text: &str) -> Vec<(ByteSpan, SymbolRole)>` — Compute Layer 0 structural tokens.

### `src/lib.rs`
- modules:
  - `pub mod lsp;`
  - `pub mod document;`
  - `pub mod proof_session;`
  - `pub mod span_conversion;`
  - `pub mod goals_panel;`
  - `pub mod edgelord_pretty_ctx;`
  - `pub mod explain;`
  - `pub mod tactics;`
  - `pub mod diff;`
  - `pub mod proposal;`
  - `pub mod loogle;`
  - `pub mod refute;`
  - `pub mod highlight;`
  - `pub mod caching;`
  - `pub mod queries;`
  - `pub mod db_memo;`

### `src/loogle/applicability.rs`
- items:
  - (pub struct) `pub struct ApplicabilityResult` — Result of checking applicability of a lemma to a goal
  - (pub fn) `pub fn check_applicability(` — Check if a lemma result is applicable to a current goal
  - (pub fn) `pub fn to_proposal(` — Convert a LoogleResult with applicability check into a Proposal
  - (pub struct) `pub struct LemmaPayload`

### `src/loogle/context.rs`
- items:
  - (pub struct) `pub struct GoalContext` — Context information about the current goal being proved
  - (pub fn) `pub fn new(goal_fingerprint: String) -> Self` — Create a new goal context
  - (pub fn) `pub fn with_bindings(mut self, bindings: Vec<String>) -> Self` — Add surrounding bindings
  - (pub fn) `pub fn with_cursor(mut self, line: usize, col: usize) -> Self` — Add cursor position
  - (pub fn) `pub fn relevance_score(&self, lemma: &LoogleResult) -> f32` — Compute relevance score for a lemma in this context Returns a score between 0.0 and 1.0 indicating how relevant the lemma is to the current goal context.
  - (pub fn) `pub fn rank_results(&self, results: Vec<LoogleResult>) -> Vec<(LoogleResult, f32)>` — Filter and sort lemma results by relevance to this context

### `src/loogle/indexer.rs`
- items:
  - (pub const) `pub const LOOGLE_FP_VERSION: u32 = 1;` — Fingerprint format version - increment when changing fingerprint structure to invalidate stale indexes and prevent version drift.
  - (pub struct) `pub struct WorkspaceIndexer` — Extracts and indexes lemmas from a workspace bundle
  - (pub fn) `pub fn new() -> tantivy::Result<Self>`
  - (pub fn) `pub fn reindex(&self, bundle: &CoreBundleV0) -> tantivy::Result<()>` — Re-index the entire workspace from a new bundle
  - (pub fn) `pub fn index(&self) -> &LoogleIndex` — Get the underlying index for search operations
  - (pub fn) `pub fn compute_fingerprint(term: &tcb_core::ast::MorphismTerm) -> String` — Compute a structural fingerprint for search indexing This produces a canonical, deterministic string representation of the term structure that can be used for structural search. The format captures term shape while abstracting over specific IDs. Format: `v{VERSION}:{fingerprint}` **Invariant G6**: No Debug-derived strings used in fingerprints. All formatting uses stable, versioned accessors.

### `src/loogle/mod.rs`
- modules:
  - `pub mod indexer;`
  - `pub mod applicability;`
  - `pub mod code_actions;`
  - `pub mod context;`
- items:
  - (pub struct) `pub struct LoogleIndex`
  - (pub fn) `pub fn new_in_memory() -> tantivy::Result<Self>`
  - (pub fn) `pub fn index_lemma(` — Index a lemma from the workspace bundle
  - (pub fn) `pub fn search(&self, query_fp: &str, limit: usize) -> tantivy::Result<Vec<LoogleResult>>` — Search for lemmas by structural fingerprint
  - (pub fn) `pub fn clear(&self) -> tantivy::Result<()>` — Clear all indexed lemmas (for re-indexing)
  - (pub struct) `pub struct LoogleResult`

### `src/lsp.rs`
- items:
  - (pub struct) `pub struct DebouncedDocumentChange`
  - (pub struct) `pub struct Config`
  - (pub fn) `pub fn document_diagnostics_from_report(`
  - (pub fn) `pub fn workspace_error_report(err: &SurfaceError) -> WorkspaceReport`
  - (pub struct) `pub struct PublishDiagnosticsHandler;` — PublishDiagnosticsHandler - Centralized diagnostic publishing system This handler is responsible for: - Converting SniperDB diagnostics to LSP format - Using the span conversion system for precise UTF-16 positions - Sorting diagnostics deterministically # Requirements - Validates: Requirements 5.1, 5.3
  - (pub fn) `pub async fn publish_diagnostics(` — Publish diagnostics for a document Converts diagnostics from various sources (parser, workspace report, SniperDB) to LSP format and publishes them via the LSP client. # Requirements - Validates: Requirements 5.1, 5.3
  - (pub fn) `pub async fn publish_preconverted(` — Publish pre-converted diagnostics This method is for cases where diagnostics have already been converted to LSP format (e.g., when merging external command diagnostics with proof session diagnostics). The diagnostics should already be sorted using `sort_diagnostics()`. # Requirements - Validates: Requirements 5.1, 5.3
  - (pub fn) `pub fn convert_diagnostics(` — Convert diagnostics from internal format to LSP format This function: 1. Converts parser diagnostics 2. Converts workspace report diagnostics 3. Sorts all diagnostics deterministically # Requirements - Validates: Requirements 5.1, 5.3
  - (pub fn) `pub fn sort_diagnostics(uri: &Url, diagnostics: &mut Vec<Diagnostic>)` — Sort diagnostics deterministically Diagnostics are sorted by: 1. URI 2. Reliability (spanned diagnostics before spanless) 3. Severity (errors before warnings before info) 4. Position (line, then character) 5. Message 6. Code 7. Source # Requirements - Validates: Requirements 5.3
  - (pub struct) `pub struct Backend`
  - (pub fn) `pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type`

### `src/queries/check_unit.rs`
- items:
  - (pub struct) `pub struct CompileInputV1` — Phase 1.2B: Canonical compilation input for unit check query All fields are deterministically serialized to create a stable input hash. No hidden non-determinism (e.g., no paths, no timestamps, no Debug strings). Components: 1. **unit_content**: Source code bytes 2. **compile_options**: Pretty printer dialect, feature flags, etc. 3. **workspace_snapshot**: All open documents (conservative dependency model) 4. **file_id**: Stable file identifier (CRC32 of URI) Serialization: Canonical byte ordering (sorted collections, explicit separators)
  - (pub fn) `pub fn compute_digest(` — Compute input digest from canonical serialization
  - (pub fn) `pub fn new(` — Create a new CompileInputV1 with computed digest
  - (pub struct) `pub struct Q_CHECK_UNIT_V1;` — Phase 1.2B: Named query for unit compilation Query: Q_CHECK_UNIT_V1 Input: CompileInputV1 (deterministically serialized) Output: DiagnosticsArtifactV1 (compilation results) Guarantee: For any given input_digest, always returns the same output. Storage: Results persisted in SniperDB's memo table by input_digest.
  - (pub const) `pub const NAME: &'static str = "Q_CHECK_UNIT_V1";`
  - (pub fn) `pub fn name() -> &'static str` — Canonical query name for logging and introspection
  - (pub fn) `pub fn query_class() -> &'static str` — Query class (e.g., "incremental_check", "unit_compile")
  - (pub fn) `pub fn input_version() -> u32` — Expected input type version
  - (pub fn) `pub fn output_version() -> u32` — Expected output type version
  - (pub struct) `pub struct DiagnosticsArtifactV1` — Phase 1.2B: Compilation output artifact for Q_CHECK_UNIT_V1 Captures the canonical output of unit compilation: - WorkspaceReport: Type information, proof state, diagnostics - Computed diagnostics: Projected to LSP format - Timestamp: When this artifact was computed Guarantee: Deterministic given the input. No side effects or mutations.
  - (pub fn) `pub fn new(` — Create a new diagnostics artifact with optional soundness proof
  - (pub fn) `pub fn verify_determinism(&self, expected_digest: &HashValue) -> bool` — Verify that output is deterministic (optional: compare with expected digest)

### `src/queries/mod.rs`
- modules:
  - `pub mod check_unit;`

### `src/refute/lsp_handler.rs`

LSP handler for edgelord/refute endpoint.

This module provides the stable JSON contract for refutation.

- items:
  - (pub const) `pub const REFUTE_PROTOCOL_VERSION: u32 = 1;` — Version of the refute protocol. Bump on breaking changes.
  - (pub struct) `pub struct RefuteRequest` — Request for refutation.
  - (pub struct) `pub struct RefuteResponse` — Response envelope for refutation. **Invariant**: Field names and ordering are stable. Bump `version` on breaking changes.
  - (pub struct) `pub struct RefuteMeta` — Metadata about the refutation request.
  - (pub fn) `pub fn sort_proposals(proposals: &mut [Proposal<CounterexamplePayload>])` — Sort proposals deterministically. Order by: status, level, probe, score (desc), failure fingerprint, id
  - (pub fn) `pub fn generate_proposal_id(anchor: &str, probe: &ProbeKey, witness: &FailureWitness) -> String` — Generate deterministic proposal ID from (anchor, probe, witness fingerprint).
  - (pub fn) `pub fn handle_refute_request(` — Handle a refute request and return a stable response. **Invariant**: Output is byte-for-byte deterministic for same input.

### `src/refute/mod.rs`

Mac Lane-native refutation engine.

This module provides doctrine-agnostic refutation via probe doctrines.
Probes enumerate interpretations and check whether obligations hold.

# Architecture

- `types`: Core bounded types (BoundedList, RefuteLimits)
- `slice`: Theory slice extraction from ProofState
- `probe`: ProbeDoctrine trait and check results
- `witness`: Counterexample and FailureWitness types
- `orchestrator`: Refuter that tries probes in order
- `probes/`: MVP probe implementations
- `render`: Witness rendering via PrettyCtx

- modules:
  - `pub mod types;`
  - `pub mod slice;`
  - `pub mod probe;`
  - `pub mod witness;`
  - `pub mod orchestrator;`
  - `pub mod probes;`
  - `pub mod render;`
  - `pub mod lsp_handler;`

### `src/refute/orchestrator.rs`

Refuter orchestrator.

Tries probes in deterministic priority order and returns the
first valid counterexample as a Proposal.

- items:
  - (pub struct) `pub struct RefuteRequest` — Request for refutation.
  - (pub struct) `pub struct Refuter` — Main refuter orchestrator. **Invariants**: - Deterministic probe ordering - Returns Proposal<CounterexamplePayload>, not bespoke result - ProposalStatus::Advisory always for Phase 10
  - (pub fn) `pub fn new(probes: Vec<Arc<dyn ProbeDoctrine>>) -> Self` — Create a new refuter with the given probes.
  - (pub fn) `pub fn with_default_probes() -> Self` — Create a refuter with default MVP probes.
  - (pub fn) `pub fn refute(` — Run refutation and return a Proposal. **Semantics**: - Tries probes in deterministic order - Returns first counterexample found - Returns "no counterexample within bounds" if nothing found

### `src/refute/probe.rs`

Probe trait and core probe types.

A probe doctrine is a small semantic world we can map into and evaluate.
This is the Mac Lane-native approach: interpretations are first-class objects.

- items:
  - (pub struct) `pub struct ProbeKey` — Stable identity for a probe doctrine. **Mac Lane move**: includes deformation level and semantic description for education + ML training data.
  - (pub struct) `pub struct InterpretationCandidate` — A candidate interpretation from a probe doctrine. Stored structurally; rendered via PrettyCtx at the boundary.
  - (pub enum) `pub enum InterpretationData` — Probe-specific interpretation data.
  - (pub enum) `pub enum RefuteCheckResult` — Result of checking an interpretation. **Critical invariant**: `NoFailureFoundWithinBounds` is NOT a counterexample. Only `FoundFailure` with a witness counts as refutation.
  - (pub trait) `pub trait ProbeDoctrine: Send + Sync` — A probe doctrine for Mac Lane-native refutation. Probes enumerate interpretations and check whether obligations hold. This is doctrine-agnostic: the same interface works for rewrite probes, finite category probes, and (eventually) SMT probes.

### `src/refute/probes/finite_cat_probe.rs`

Finite skeletal category probe (MVP Probe B).

Checks obligations in small finite categories (N≤3 default).
Returns UnsupportedFragment if categorical presentation not extractable.

- items:
  - (pub struct) `pub struct FiniteCatProbe;` — Finite skeletal category probe. **Semantics**: - Domain: objects 0..N-1 - Morphisms: generated by slice with explicit composition - Checks: composition, identities, associativity - Returns `UnsupportedFragment` if categorical presentation not extractable
  - (pub fn) `pub fn new() -> Self`

### `src/refute/probes/mod.rs`

Probe implementations submodule.

- modules:
  - `pub mod rewrite_probe;`
  - `pub mod finite_cat_probe;`

### `src/refute/probes/rewrite_probe.rs`

Rewrite obstruction probe (MVP Probe A).

Detects failures via rewriting: non-joinable peaks, critical pairs.
Returns honest results: only FoundFailure when genuine obstruction found.

- items:
  - (pub struct) `pub struct RewriteProbe;` — Rewrite obstruction probe. **Semantics**: - Treats slice as a rewriting system - Searches for non-joinable peaks / critical pairs - Returns `NoFailureFoundWithinBounds` on timeout (NOT a false refutation)
  - (pub fn) `pub fn new() -> Self`

### `src/refute/render.rs`

Witness rendering (MVVM separation).

Keeps witnesses structural; renders to pretty text at the boundary.

- items:
  - (pub fn) `pub fn render_counterexample(cx: &Counterexample) -> String` — Render a counterexample to human-readable text.
  - (pub fn) `pub fn render_failure_witness(witness: &FailureWitness) -> String` — Render a failure witness to human-readable text.
  - (pub struct) `pub struct RefuteExplanationNode` — A simplified ExplanationNode for refutation witnesses. This mirrors the structure in `explain::view` but is standalone for the refute module to avoid circular dependencies.
  - (pub fn) `pub fn leaf(label: String, kind: &str) -> Self`
  - (pub fn) `pub fn with_jump(mut self, start: usize, end: usize) -> Self`
  - (pub fn) `pub fn witness_to_explanation_tree(witness: &FailureWitness) -> RefuteExplanationNode` — Convert a failure witness to an explanation tree. **Invariant**: All labels PrettyCtx-rendered, jump targets byte-offset. UTF-16 conversion happens at LSP boundary via `byte_to_utf16_offset`.
  - (pub fn) `pub fn validate_tree_spans(node: &RefuteExplanationNode, text_len: usize) -> bool` — Validate that all jump targets in a tree are within bounds. Returns `true` if all spans are valid.

### `src/refute/slice.rs`

Refutation slice extraction.

A RefuteSlice is a minimal, trace-driven theory slice containing:
- Target obligation(s)
- Minimal blocker set
- Bounded neighborhood of rules/defs from trace graph

- items:
  - (pub struct) `pub struct RefuteSlice` — A minimal theory slice for refutation. **Key invariants**: - `anchor` is structured (not String) - Obligations sorted deterministically - Rules/defs bounded by trace neighborhood
  - (pub struct) `pub struct Obligation` — An obligation to check in the refutation.
  - (pub struct) `pub struct TermRef` — Reference to a term (structural, rendered via PrettyCtx).
  - (pub struct) `pub struct RuleRef` — Reference to a rule in the slice.
  - (pub struct) `pub struct DefRef` — Reference to a definition in the slice.
  - (pub struct) `pub struct CoherenceObligation` — A coherence obligation for level≥1 proofs. When coherence level > 0, the slice includes diagram boundaries that need to be checked or surfaced as "missing coherence" if undecidable.
  - (pub struct) `pub struct DiagramBoundary` — A diagram boundary requiring a higher cell.
  - (pub struct) `pub struct SliceSummary` — Summary of what was included in the slice. Explicitly educational: shows what was assumed for the refutation.
  - (pub fn) `pub fn from_slice(slice: &RefuteSlice, trace_steps: usize) -> Self` — Create summary from a slice.
  - (pub fn) `pub fn extract_slice(` — Extract a minimal slice for refutation. **Policy**: - Start from target goal/constraint - Include minimal blockers (from GoalsIndex) - Include rules/defs from trace neighborhood up to max_trace_steps - Deterministic sort everywhere

### `src/refute/types.rs`

Core types for Mac Lane-native refutation engine.

These types follow the universal bounded list schema and use structured
internal representations (no pre-rendered strings).

- items:
  - (pub struct) `pub struct StableAnchor` — A stable identifier for a proof artifact. This is a local copy to avoid deep dependency paths. Matches the structure in `new_surface_syntax::diagnostics::anchors`.
  - (pub enum) `pub enum AnchorKind`
  - (pub fn) `pub fn to_id_string(&self) -> String` — Compute the deterministic string ID.
  - (pub fn) `pub fn test(kind: AnchorKind, file: &str, ordinal: u32) -> Self` — Create a test anchor.
  - (pub struct) `pub struct BoundedList<T>` — A bounded list with full truncation tracking. **Invariant**: This is the universal bounded list type used across EdgeLorD (refute, explain, loogle). Always includes: - `total_count`: how many items existed before capping - `truncation_reason`: why we stopped (if truncated)
  - (pub fn) `pub fn from_vec(items: Vec<T>) -> Self` — Create a non-truncated list.
  - (pub fn) `pub fn truncated(items: Vec<T>, total_count: usize, reason: TruncationReason) -> Self` — Create a truncated list.
  - (pub fn) `pub fn empty() -> Self` — Create an empty list.
  - (pub enum) `pub enum TruncationReason` — Reason for truncation in bounded operations.
  - (pub struct) `pub struct DecisionInfo` — Uniform decision envelope for every witness. This prevents clients from having to interpret enum variants as policy.
  - (pub fn) `pub fn decided() -> Self` — Decided failure within decidable fragment.
  - (pub fn) `pub fn not_found(reason: &str) -> Self` — Decidable but no failure found.
  - (pub fn) `pub fn undecidable(reason: &str) -> Self` — Not decidable by this probe.
  - (pub struct) `pub struct JumpTarget` — A structured jump target for explain/UI integration.
  - (pub fn) `pub fn from_span(start: usize, end: usize) -> Self`
  - (pub fn) `pub fn with_label(mut self, label: &str) -> Self`
  - (pub fn) `pub fn with_kind(mut self, kind: &str) -> Self`
  - (pub struct) `pub struct ByteSpan` — Byte span for jump targets.
  - (pub struct) `pub struct RefuteLimits` — Resource limits for refutation. Defaults are conservative for fast p95 response times.
  - (pub enum) `pub enum RefuteFragment` — Fragment of theory that a probe can handle. Used for "honest unsupported" - probes decline fragments they can't check.
  - (pub fn) `pub fn level(&self) -> u8` — The coherence level this fragment requires.

### `src/refute/witness.rs`

Witness types for refutation failures.

Witnesses are pedagogical artifacts, not solver dumps.
They're stored structurally and rendered via PrettyCtx.

- items:
  - (pub struct) `pub struct Counterexample` — A counterexample found by refutation. This is the payload for `Proposal<CounterexamplePayload>`.
  - (pub struct) `pub struct InterpretationSummary` — Summary of an interpretation for display.
  - (pub enum) `pub enum InterpretationKind` — Tagged interpretation kind for forward compatibility.
  - (pub enum) `pub enum FailureWitness` — A witness to a failure in the interpretation. **Mac Lane-native**: This is not a "model" - it's a failed obligation described in the language of proof objects (explainable + teachable).
  - (pub struct) `pub struct DiagramBoundary` — A diagram boundary requiring a higher cell. Machine-facing structure for UI, explain integration, and diagram workflows.
  - (pub struct) `pub struct CounterexamplePayload` — Payload for `Proposal<CounterexamplePayload>`. Wraps Counterexample with additional context for the proposal protocol.
  - (pub fn) `pub fn new(counterexample: Counterexample) -> Self`

### `tests/bench_phase1_cache.rs`

### `tests/bench_phase1_report.rs`
- items:
  - (pub struct) `pub struct Local;`
  - (pub fn) `pub fn now() -> String`

### `tests/cache_phase1_1_bench.rs`

### `tests/cache_phase1_1_invariants.rs`

### `tests/cache_phase1_1_races.rs`

### `tests/db7_hover_preview.rs`

### `tests/diagnostic_publishing_tests.rs`

### `tests/highlight_test.rs`

### `tests/loogle_tests.rs`

### `tests/phase1_2b_compile_query.rs`

### `tests/phase1_module_snapshots.rs`

### `tests/refute_golden_check.rs`

### `tests/refute_pipeline_integration.rs`

### `tests/refute_tests.rs`

Golden determinism tests for refute endpoint (G5).

These tests lock the JSON schema and ensure byte-for-byte determinism.


### `tests/smoke_test.rs`
