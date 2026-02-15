# RAG Semantic Bundle (repo-safe)

- Repo root: `/Users/e/Documents/MotivicCohomology/GitLocal/EdgeLorD`
- Git HEAD: `6ecfa20952749bb57cb096dacccd1eabbb4cdec9` (dirty)
- Generated (UTC): `2026-02-10T03:32:16.846771+00:00`
- Mode: `overview`

## Audit
- considered (git paths): **153**
- included: **135**
- skipped: not-allowed=18, excluded=0, missing=0, too-large=0
- budgets: max_files=1000000, max_file_bytes=5000000, max_output_kb=200000

## Cargo overview

### `Cargo.toml`
- package: **edgelord-lsp** 0.1.0
- deps (16): tokio, tower-lsp, serde, serde_json, thiserror, source_span, codeswitch, comrade_lisp, sniper_db, async-trait, crc32fast, tantivy, tcb_core, sha2, hex, bincode

## Rust semantics (lossy)

- Included Rust files: **77**
- include_private=1; brief=0

### `src/caching.rs`
- uses:
  - `std::collections::BTreeMap`
  - `std::fmt`
  - `std::time::SystemTime`
  - `std::sync::{Arc, RwLock}`
  - `serde::{Serialize, Deserialize, Serializer, Deserializer}`
  - `bincode`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `tower_lsp::lsp_types::Diagnostic`
  - `codeswitch::fingerprint::HashValue`
  - `sniper_db::SniperDatabase`
  - `serde::ser::SerializeStruct`
  - `serde::de::{self, MapAccess, Visitor}`
  - `std::fmt`
  - `super::*`
- items:
  - (pub trait) `pub trait SnapshotStore: Send + Sync` — Minimal snapshot storage interface (L1/L2 abstraction) L1 (InMemoryStore): Fast in-process cache (BTreeMap LRU) L2 (SniperDbStore): Persistent backing (survives restarts) ModuleSnapshotCache checks L1 first, then L2, then compiles.
  - (priv fn) `fn get(&self, key: &SnapshotStoreKey) -> Option<SerializedSnapshot>;`
  - (priv fn) `fn put(&self, key: SnapshotStoreKey, snapshot: &SerializedSnapshot);`
  - (pub struct) `pub struct SnapshotStoreKey(pub HashValue);` — Key for snapshot storage: derive from 4-component cache key
  - (priv impl) `impl SnapshotStoreKey`
  - (pub fn) `pub fn from_cache_key(` — Create from the 4-component cache key
  - (pub struct) `pub struct SerializedSnapshot` — Serialized snapshot for storage (compact form, Phase 1.2B: deferred serialization)
  - (priv impl) `impl Serialize for SerializedSnapshot`
  - (priv fn) `fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>`
  - (priv fn) `fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>`
  - (priv enum) `enum Field`
  - (priv struct) `struct SerializedSnapshotVisitor;`
  - (priv type) `type Value = SerializedSnapshot;`
  - (priv fn) `fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result`
  - (priv fn) `fn visit_map<V>(self, mut map: V) -> Result<SerializedSnapshot, V::Error>`
  - (priv impl) `impl SerializedSnapshot`
  - (pub fn) `pub fn from_module_snapshot(snapshot: &ModuleSnapshot) -> Self`
  - (pub fn) `pub fn to_module_snapshot(`
  - (pub struct) `pub struct InMemorySnapshotStore` — L1 (in-memory) snapshot store
  - (priv impl) `impl InMemorySnapshotStore`
  - (pub fn) `pub fn new() -> Self`
  - (priv impl) `impl Default for InMemorySnapshotStore`
  - (priv fn) `fn default() -> Self`
  - (priv impl) `impl SnapshotStore for InMemorySnapshotStore`
  - (priv fn) `fn get(&self, key: &SnapshotStoreKey) -> Option<SerializedSnapshot>`
  - (priv fn) `fn put(&self, key: SnapshotStoreKey, snapshot: &SerializedSnapshot)`
  - (pub struct) `pub struct SniperDbSnapshotStore` — L2 (persistent) snapshot store backed by SniperDatabase
  - (priv impl) `impl std::fmt::Debug for SniperDbSnapshotStore`
  - (priv fn) `fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result`
  - (priv impl) `impl SniperDbSnapshotStore`
  - (pub fn) `pub fn new(db: Arc<SniperDatabase>) -> Self`
  - (priv impl) `impl SnapshotStore for SniperDbSnapshotStore`
  - (priv fn) `fn get(&self, key: &SnapshotStoreKey) -> Option<SerializedSnapshot>`
  - (priv fn) `fn put(&self, key: SnapshotStoreKey, snapshot: &SerializedSnapshot)`
  - (pub enum) `pub enum Phase1MissReason` — Phase 1 (Module Snapshot) cache miss reasons
  - (priv impl) `impl Phase1MissReason`
  - (pub fn) `pub fn to_outcome_string(&self) -> &'static str` — Convert to outcome string for CSV
  - (pub enum) `pub enum Phase1_1MissReason` — Phase 1.1 (Workspace-Aware) cache miss reasons
  - (priv impl) `impl Phase1_1MissReason`
  - (pub fn) `pub fn to_outcome_string(&self) -> &'static str` — Convert to outcome string for CSV
  - (pub enum) `pub enum CacheOutcome<M>` — Cache outcome: either a hit or a miss with reason
  - (pub fn) `pub fn to_outcome_string(&self) -> String` — Convert to outcome string for CSV
  - (pub trait) `pub trait CacheOutcomeString` — Trait for types that can be converted to outcome strings
  - (priv fn) `fn to_outcome_string(&self) -> &'static str;`
  - (priv impl) `impl CacheOutcomeString for Phase1MissReason`
  - (priv fn) `fn to_outcome_string(&self) -> &'static str`
  - (priv impl) `impl CacheOutcomeString for Phase1_1MissReason`
  - (priv fn) `fn to_outcome_string(&self) -> &'static str`
  - (pub enum) `pub enum CacheGetResult<V>` — C2.4: Cache lookup result - refactor-safe outcome classification Instead of counter-based or log-based hit detection, the cache lookup directly returns whether it was a hit or miss with a classified reason. This makes outcome determination part of the control flow and immune to refactoring.
  - (pub enum) `pub enum CacheGetResult1_1<V>`
  - (pub fn) `pub fn to_outcome_string(&self) -> String`
  - (pub fn) `pub fn is_hit(&self) -> bool`
  - (pub fn) `pub fn to_outcome_string(&self) -> String`
  - (pub fn) `pub fn is_hit(&self) -> bool`
  - (pub struct) `pub struct CacheKeyBuilder` — Builder for CacheKey with safe defaults and validation.
  - (priv impl) `impl CacheKeyBuilder`
  - (pub fn) `pub fn new() -> Self`
  - (pub fn) `pub fn options(mut self, fp: HashValue) -> Self`
  - (pub fn) `pub fn workspace_snapshot(mut self, hash: HashValue) -> Self`
  - (pub fn) `pub fn unit_id(mut self, id: impl Into<String>) -> Self`
  - (pub fn) `pub fn unit_content(mut self, hash: HashValue) -> Self`
  - (pub fn) `pub fn dependencies(mut self, fp: HashValue) -> Self`
  - (pub fn) `pub fn build(self) -> Result<CacheKey, String>`
  - (priv impl) `impl Default for CacheKeyBuilder`
  - (priv fn) `fn default() -> Self`
  - (pub struct) `pub struct CacheKey` — Canonical cache key combining all inputs that affect compilation. As per PHASE_1_1_ACCEPTANCE_SPEC.md §1.1: - OptionsFingerprint: hash of canonical serialization of compile options (BTreeMap/sorted) - WorkspaceSnapshotHash: hash of workspace state (open docs + dependencies) - UnitId: canonicalized file identifier (prefer FileId, normalize Url if needed) - UnitContentHash: hash of file content at compilation time - DependencyFingerprint: hash of transitive dependencies INV D-CACHE-1 precondition: "same options" means bit-for-bit identical canonical bytes
  - (priv impl) `impl fmt::Display for CacheKey`
  - (priv fn) `fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result`
  - (pub struct) `pub struct CacheValue` — Cached compilation output and associated metadata.
  - (pub struct) `pub struct ModuleSnapshot` — Phase 1: Module snapshot backed by SniperDB A module snapshot captures the compilation result for a single file with a COMPLETE key that includes all inputs affecting compilation: - file_id: Deterministic file identifier - content_hash: Hash of file content - options_fingerprint: Hash of compile options - dependency_fingerprint: Hash of transitive dependencies INV PHASE-1-MODULE-1 (Sound Reuse): Only reuse when ALL inputs match INV PHASE-1-MODULE-2 (Complete Key): Must include options, dependencies, content INV PHASE-1-MODULE-3 (Content Stability): Same inputs → same snapshot, always
  - (pub struct) `pub struct ModuleSnapshotCache` — Phase 1.2A: Module snapshot cache with L1/L2 storage Implements a 2-level cache: - L1: In-memory BTreeMap LRU (fast, hot) - L2: SniperDB persistent store (survives restarts, auditable) Tracks snapshots by a COMPLETE 4-component key: (file_id, content_hash, options_fingerprint, dependency_fingerprint) This ensures sound reuse: only reuse when ALL compilation inputs match. Prevents silent reuse of stale data when dependencies or options change. Strategy (L1/L2): 1. Check L1 in-memory cache (fast path) 2. Check L2 SniperDB persistent store (cross-session reuse) 3. Miss: return None, compute fresh 4. L2 hit: promote to L1 for next access Returns None on cache miss (any component mismatch = miss).
  - (pub struct) `pub struct ModuleSnapshotStats` — Statistics for Phase 1 module snapshot cache
  - (priv impl) `impl ModuleSnapshotStats`
  - (pub fn) `pub fn hit_rate(&self) -> f64`
  - (pub fn) `pub fn record_hit(&mut self)`
  - (pub fn) `pub fn record_miss(&mut self)`
  - (priv impl) `impl ModuleSnapshotCache`
  - (pub fn) `pub fn new(db: Arc<SniperDatabase>) -> Self` — Create a new module snapshot cache with L1 (in-memory) and L2 (SniperDB) backing.
  - (pub fn) `pub fn with_max_entries(db: Arc<SniperDatabase>, max_entries: usize) -> Self` — Create with custom max entries.
  - (pub fn) `pub fn get(` — Retrieve module snapshot with a COMPLETE 4-component key (L1/L2 lookup). INV PHASE-1-MODULE-1: Only reuse when all inputs match - file_id: Source file identifier - content_hash: File content hash - options_fingerprint: Compile options hash - dependency_fingerprint: Transitive dependencies hash Phase 1.2A: L1/L2 lookup strategy 1. Check L1 (in-memory BTreeMap) - fast, hot 2. Check L2 (SniperDB persistent store) - cross-session reuse 3. Miss: return None, compute fresh 4. L2 hit: promote to L1 for next access Returns None on cache miss (any component mismatch = miss).
  - (pub fn) `pub fn insert(&mut self, snapshot: ModuleSnapshot)` — Insert a module snapshot into the cache with complete key (write-through L1/L2). Snapshot must include all input components (content_hash, options_fingerprint, dependency_fingerprint) to ensure sound reuse. Phase 1.2A: Write-through strategy 1. Insert into L1 (in-memory BTreeMap) 2. Insert into L2 (SniperDB persistent store) 3. Evict oldest L1 entries if exceeding max_entries This ensures cross-session reuse and makes caching auditable.
  - (pub fn) `pub fn stats(&self) -> ModuleSnapshotStats` — Get current statistics.
  - (pub fn) `pub fn clear(&mut self)` — Clear all cached snapshots (L1 only; L2 persists).
  - (pub fn) `pub fn len(&self) -> usize` — Get L1 (in-memory) cache size.
  - (pub fn) `pub fn is_empty(&self) -> bool` — Check if L1 cache is empty.
  - (pub fn) `pub fn reset_stats(&mut self)` — Reset statistics (for testing).
  - (pub struct) `pub struct CacheStats` — Statistics for cache hits/misses and performance analysis.
  - (priv impl) `impl CacheStats`
  - (pub fn) `pub fn hit_rate(&self) -> f64`
  - (pub fn) `pub fn total_operations(&self) -> u64`
  - (pub fn) `pub fn record_miss(&mut self, reason: impl Into<String>)`
  - (pub fn) `pub fn record_hit(&mut self)`
  - (pub trait) `pub trait CacheStore: Send + Sync` — CacheStore trait: abstraction over storage backends (memory, SniperDB, etc.) Phase 1.2 + Phase 2A: Support pluggable backends while keeping correctness invariants. All backends must preserve: - INV D-CACHE-2 (Sound reuse): identical key → identical cached value - Single-flight semantics: operations atomic with respect to compilation gate
  - (priv fn) `fn get(&self, key: &CacheKey) -> Option<CacheValue>;` — Retrieve a cached value for the given key, if present.
  - (priv fn) `fn put(&self, key: CacheKey, value: CacheValue);` — Store a compiled value under the given key.
  - (priv fn) `fn stats(&self) -> CacheStats;` — Get current statistics (hits, misses, etc.)
  - (priv fn) `fn clear(&self);` — Clear all cached entries (for testing or reset).
  - (pub struct) `pub struct InMemoryCacheStore` — InMemoryCacheStore: Thread-safe in-memory cache implementation. Invariants: - INV D-CACHE-1 (Purity): Same key → identical output, always - INV D-CACHE-2 (Sound reuse): Reuse only when key exactly matches - INV D-CACHE-3 (Monotone invalidation): Edits invalidate affected caches Thread-safety: Internally uses Arc<RwLock<>> to allow safe concurrent access. Cache must be accessed inside single-flight gate to preserve no-stale-diagnostics.
  - (priv struct) `struct InMemoryCacheStoreInner`
  - (pub type) `pub type ModuleCache = InMemoryCacheStore;` — Backward-compatible alias for existing code
  - (priv impl) `impl InMemoryCacheStore`
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
  - (priv impl) `impl CacheStore for InMemoryCacheStore`
  - (priv fn) `fn get(&self, key: &CacheKey) -> Option<CacheValue>`
  - (priv fn) `fn put(&self, key: CacheKey, value: CacheValue)`
  - (priv fn) `fn stats(&self) -> CacheStats`
  - (priv fn) `fn clear(&self)`
  - (priv impl) `impl Default for InMemoryCacheStore`
  - (priv fn) `fn default() -> Self`
  - (priv fn) `fn test_cache_key_ordering()`
  - (priv fn) `fn test_cache_hit_miss_stats()`
  - (priv fn) `fn test_cache_eviction()`
  - (priv fn) `fn test_cache_determinism()`
  - (priv fn) `fn test_cache_key_exact_match()`
  - (priv fn) `fn test_module_snapshot_cache_hit_on_all_inputs_match()`
  - (priv fn) `fn test_module_snapshot_cache_miss_on_content_change()`
  - (priv fn) `fn test_module_snapshot_cache_miss_on_deps_change()`
  - (priv fn) `fn test_module_snapshot_cache_miss_on_options_change()`
  - (priv fn) `fn test_module_snapshot_cache_eviction()`
  - (priv fn) `fn test_module_snapshot_stats()`
  - (priv fn) `fn test_module_snapshot_fingerprint_completeness_options()`
  - (priv fn) `fn test_options_fingerprint_guards_against_missing_fields()`
  - (priv fn) `fn test_options_fingerprint_captures_feature_flags()`
  - (priv fn) `fn test_module_snapshot_fingerprint_completeness_deps_transitive()`

### `src/client_sink.rs`
- uses:
  - `async_trait::async_trait`
  - `tower_lsp::Client`
- items:
  - (pub trait) `pub trait ClientSink: Send + Sync + 'static`
  - (priv fn) `async fn log_message(&self, message_type: MessageType, message: String);`
  - (priv fn) `async fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>, version: Option<i32>);`
  - (pub struct) `pub struct RealClientSink`
  - (priv impl) `impl RealClientSink`
  - (pub fn) `pub fn new(client: Client) -> Self`
  - (priv impl) `impl ClientSink for RealClientSink`
  - (priv fn) `async fn log_message(&self, message_type: MessageType, message: String)`
  - (priv fn) `async fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>, version: Option<i32>)`

### `src/db_memo.rs`
- uses:
  - `std::sync::Arc`
  - `sniper_db::SniperDatabase`
  - `codeswitch::fingerprint::HashValue`
  - `crate::queries::{CompileInputV1, DiagnosticsArtifactV1, Q_CHECK_UNIT_V1}`
  - `super::*`
- items:
  - (pub struct) `pub struct DbMemo` — Phase 1.2B: DB-Native Memoization Wrapper Minimal abstraction over SniperDB for memoizing unit compilation results. This is the bridge between ProofSession and SniperDB's memo infrastructure. Core operation: `memo_get_or_compute(input) -> output` - If input hash exists in memo table, return cached output - Otherwise, call the provided compute function once - Store result in memo table - Return result Hard guarantees: - **Determinism**: Input hash uniquely determines output - **Single-flight**: Each unique input computed at most once (atomic) - **Purity**: No side effects during memo lookup/store Wrapper for SniperDB memo operations
  - (priv struct) `struct MemoKey` — Serializable format for storing CompileInputV1 in DB (Phase 1.2B: deferred)
  - (priv struct) `struct MemoValue` — Serializable format for storing DiagnosticsArtifactV1 in DB (Phase 1.2B: deferred)
  - (priv impl) `impl DbMemo`
  - (pub fn) `pub fn new(db: Arc<SniperDatabase>) -> Self` — Create a new DB memo wrapper
  - (pub fn) `pub async fn memo_get_or_compute<F, Fut>(` — Retrieve a memoized compilation result, or compute it if not found. Hard invariant: If input_digest is found in memo, the cached output is returned. Otherwise, the compute function is called exactly once and the result is memoized. This implements "single-flight" semantics: concurrent requests for the same input will coordinate through the database (SniperDB's internal locking).
  - (pub fn) `pub fn memo_put(` — Pre-compute and store a memoization result (for testing/benchmarking)
  - (pub fn) `pub fn memo_contains(&self, input: &CompileInputV1) -> bool` — Check if a result is memoized (for testing)
  - (priv fn) `fn make_memo_key(&self, input: &CompileInputV1) -> MemoKey` — Create a canonical memo key for the input
  - (pub fn) `pub fn db(&self) -> &Arc<SniperDatabase>` — Retrieve the SniperDatabase reference (for testing/debugging)
  - (priv fn) `async fn test_db_memo_get_or_compute_defers_to_compute()`

### `src/diff/engine.rs`
- uses:
  - `std::collections::{BTreeMap, BTreeSet}`
  - `comrade_lisp::proof_state::{ProofState, MorMetaId, GoalStatus as KernelGoalStatus}`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `crate::goals_panel::{GoalDelta, GoalChangeKind, GoalStatus}`
- items:
  - (pub fn) `pub fn compute_diff(` — Computes the semantic difference between two proof states.
  - (priv fn) `fn map_status(status: &KernelGoalStatus) -> GoalStatus`
  - (priv fn) `fn get_blockers(status: &KernelGoalStatus, index: &GoalsPanelIndex) -> BTreeSet<String>`

### `src/diff/mod.rs`
- modules:
  - `pub mod engine;`

### `src/document.rs`
- uses:
  - `std::{cmp::Ordering, collections::BTreeSet}`
  - `serde::{Deserialize, Serialize}`
  - `tower_lsp::lsp_types::TextDocumentContentChangeEvent`
- items:
  - (pub struct) `pub struct ByteSpan`
  - (priv impl) `impl ByteSpan`
  - (pub fn) `pub fn new(start: usize, end: usize) -> Self`
  - (pub fn) `pub fn len(&self) -> usize`
  - (pub fn) `pub fn contains_offset(&self, offset: usize) -> bool`
  - (pub struct) `pub struct ParseDiagnostic`
  - (priv struct) `struct CstNode`
  - (pub struct) `pub struct ParsedDocument`
  - (pub struct) `pub struct Goal`
  - (pub enum) `pub enum BindingKind`
  - (pub struct) `pub struct Binding`
  - (pub struct) `pub struct GoalInlayHint`
  - (priv impl) `impl ParsedDocument`
  - (pub fn) `pub fn parse(text: String) -> Self`
  - (pub fn) `pub fn selection_chain_for_offset(&self, offset: usize) -> Vec<ByteSpan>`
  - (pub fn) `pub fn goal_at_offset(&self, offset: usize) -> Option<&Goal>`
  - (pub fn) `pub fn goal_inlay_hints_in_range(&self, range: ByteSpan) -> Vec<GoalInlayHint>`
  - (priv fn) `fn node_order(nodes: &[CstNode]) -> impl FnMut(&usize, &usize) -> Ordering + '_`
  - (priv fn) `fn add_expr_node(nodes: &mut Vec<CstNode>, parent: usize, expr: &SExpr) -> usize`
  - (priv fn) `fn parse_error_span(err: &ParseError, text_len: usize) -> ByteSpan`
  - (priv fn) `fn extract_goals(forms: &[SExpr]) -> Vec<Goal>`
  - (priv fn) `fn collect_holes_in_expr(`
  - (priv fn) `fn hole_form_name(items: &[SExpr]) -> Option<String>`
  - (priv fn) `fn collect_top_level_bindings(form: &SExpr, context: &mut Vec<Binding>)`
  - (priv fn) `fn goal_id(start: usize, end: usize, name: Option<&str>) -> String`
  - (priv fn) `fn spans_intersect(a: ByteSpan, b: ByteSpan) -> bool`
  - (priv fn) `fn goal_label(goal: &Goal, text: &str) -> String`
  - (priv fn) `fn let_bindings(items: &[SExpr]) -> Option<Vec<Binding>>`
  - (priv fn) `fn merged_context(local_frames: &[Vec<Binding>], top_level_bindings: &[Binding]) -> Vec<Binding>`
  - (pub fn) `pub fn position_to_offset(text: &str, position: tower_lsp::lsp_types::Position) -> usize`
  - (pub fn) `pub fn offset_to_position(text: &str, offset: usize) -> tower_lsp::lsp_types::Position`
  - (pub fn) `pub fn apply_content_changes(text: &str, changes: &[TextDocumentContentChangeEvent]) -> String`
  - (pub fn) `pub fn selection_chain_is_well_formed(chain: &[ByteSpan]) -> bool`
  - (pub fn) `pub fn top_level_symbols(text: &str) -> Vec<(String, ByteSpan)>`

### `src/edgelord_pretty_ctx.rs`
- uses:
  - `comrade_lisp::diagnostics::pretty::{PrettyCtx, PrettyPrinter, PrettyDialect, PrettyLimits, PrinterRegistry, PrinterKey}`
  - `comrade_lisp::diagnostics::DiagnosticContext`
  - `comrade_lisp::proof_state::ProofState`
  - `tower_lsp::lsp_types::Url`
- items:
  - (pub struct) `pub struct EdgeLordPrettyCtx<'a>` — Ephemeral pretty-printing context for LSP requests. **Lifetime**: Created per hover/inlay/diagnostic request, discarded after. **Design**: Borrowed wrapper avoiding copies of large objects.
  - (pub fn) `pub fn new(`
  - (pub fn) `pub fn document_uri(&self) -> &'a Url`
  - (priv fn) `fn printer(&self) -> &dyn PrettyPrinter`
  - (priv fn) `fn proof(&self) -> &ProofState`
  - (priv fn) `fn files(&self) -> &DiagnosticContext<'_>`
  - (priv fn) `fn limits(&self) -> PrettyLimits`

### `src/explain/alg_blocked.rs`
- uses:
  - `crate::explain::builder::ExplainBuilder`
  - `crate::explain::view::{ExplanationView, ExplanationKind, ExplainLimits}`
  - `comrade_lisp::proof_state::{ProofState, GoalStatus, MorMetaId}`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `std::collections::{BTreeSet, HashMap}`
  - `source_span::Span`
  - `comrade_lisp::proof_state`
- items:
  - (pub fn) `pub fn explain_why_blocked(` — Explain why a goal is blocked: show linear blocker chains for highest impact metas.
  - (priv fn) `fn compute_meta_impact(ps: &ProofState) -> HashMap<MorMetaId, usize>`
  - (priv fn) `fn find_goal_for_meta(ps: &ProofState, meta_id: MorMetaId) -> Option<&proof_state::GoalState>`
  - (priv fn) `fn find_span_for_meta(ps: &ProofState, meta_id: MorMetaId) -> Option<Span>`
  - (priv fn) `fn find_primary_blocker(ps: &ProofState, meta_id: MorMetaId, impact_map: &HashMap<MorMetaId, usize>) -> Option<MorMetaId>`

### `src/explain/alg_goal.rs`
- uses:
  - `crate::explain::builder::ExplainBuilder`
  - `crate::explain::view::{ExplanationView, ExplanationKind, ExplainLimits}`
  - `comrade_lisp::proof_state::{ProofState, GoalStatus}`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `comrade_lisp::proof_state`
- items:
  - (pub fn) `pub fn explain_goal(` — Explain what a goal is: its context, target, and direct evidence.
  - (priv fn) `fn find_span_for_meta(ps: &ProofState, meta_id: proof_state::MorMetaId) -> Option<source_span::Span>`

### `src/explain/alg_inconsistent.rs`
- uses:
  - `crate::explain::builder::ExplainBuilder`
  - `crate::explain::view::{ExplanationView, ExplanationKind, ExplainLimits}`
  - `comrade_lisp::proof_state::{ProofState, GoalStatus}`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
- items:
  - (pub fn) `pub fn explain_why_inconsistent(` — Explain why a goal is inconsistent: show deterministic conflict sets and their origins.
  - (priv const) `const MAX_CONFLICT_SETS: usize = 3;`

### `src/explain/builder.rs`
- uses:
  - `std::collections::{BTreeMap, VecDeque, HashSet}`
  - `source_span::Span`
  - `crate::explain::view::{ExplanationNode, ExplanationKind, ExplanationView, ExplainLimits}`
- items:
  - (pub struct) `pub struct NodeRecord` — Internal record for a node in the arena.
  - (pub struct) `pub struct ExplainBuilder` — Arena-based builder to avoid borrow-check pain and ensure deterministic materialization.
  - (priv impl) `impl ExplainBuilder`
  - (pub fn) `pub fn new(limits: ExplainLimits) -> Self`
  - (pub fn) `pub fn set_root(&mut self, id: String, kind: ExplanationKind, label: String, span: Option<Span>) -> usize`
  - (pub fn) `pub fn add_metadata(&mut self, node_idx: usize, key: String, value: String)`
  - (pub fn) `pub fn add_child(&mut self, parent_idx: usize, id: String, kind: ExplanationKind, label: String, span: Option<Span>) -> Option<usize>` — Add a child to a parent node. Returns Some(child_idx) if added, None if blocked by limits or already visited.
  - (priv fn) `fn set_truncated(&mut self, reason: &str)`
  - (pub fn) `pub fn next_idx(&mut self) -> Option<usize>`
  - (pub fn) `pub fn get_node(&self, idx: usize) -> &NodeRecord`
  - (pub fn) `pub fn build(self) -> ExplanationView` — Build the final ExplanationView. Performs deterministic sorting of children before materializing.
  - (priv fn) `fn materialize_node(&self, idx: usize) -> ExplanationNode`
  - (pub fn) `pub fn truncate_label(label: String, max_chars: usize) -> String`

### `src/explain/mod.rs`
- modules:
  - `pub mod view;`
  - `pub mod builder;`
  - `pub mod alg_goal;`
  - `pub mod alg_blocked;`
  - `pub mod alg_inconsistent;`
- uses:
  - `tower_lsp::jsonrpc::Result`
  - `crate::explain::view::{ExplainRequest, ExplanationView, ExplainTarget, ExplainLimits, ExplanationNode, validate_span}`
  - `crate::proof_session::ProofSession`
  - `std::sync::Arc`
  - `tokio::sync::RwLock`
- items:
  - (pub fn) `pub async fn handle_explain_request(` — Main handler for edgelord/explain requests.
  - (priv fn) `fn enforce_hard_caps(mut limits: ExplainLimits) -> ExplainLimits`
  - (priv const) `const MAX_NODES_CAP: usize = 300;`
  - (priv const) `const MAX_DEPTH_CAP: usize = 50;`
  - (priv const) `const MAX_CHILDREN_CAP: usize = 50;`
  - (priv const) `const MAX_LABEL_CHARS_CAP: usize = 5000;`
  - (priv const) `const MAX_TIMEOUT_MS_CAP: u64 = 1000;`
  - (priv fn) `fn validate_view_spans(node: &mut ExplanationNode, text_len: usize)`

### `src/explain/view.rs`
- uses:
  - `serde::{Serialize, Deserialize}`
  - `tower_lsp::lsp_types::Url`
  - `source_span::Span`
  - `std::collections::BTreeMap`
- items:
  - (pub struct) `pub struct ExplainRequest`
  - (pub enum) `pub enum ExplainTarget`
  - (pub struct) `pub struct ExplainLimits`
  - (priv impl) `impl Default for ExplainLimits`
  - (priv fn) `fn default() -> Self`
  - (pub struct) `pub struct ExplanationView`
  - (pub struct) `pub struct ExplanationNode`
  - (pub enum) `pub enum ExplanationKind`
  - (pub fn) `pub fn validate_span(span: Span, text_len: usize) -> Option<Span>` — Helper to validate spans against text length.

### `src/goals_panel.rs`
- uses:
  - `serde::{Serialize, Deserialize}`
  - `tower_lsp::lsp_types::Range`
- items:
  - (pub struct) `pub struct GoalsPanelResponse`
  - (pub struct) `pub struct GoalPanelItem`
  - (pub enum) `pub enum GoalStatus`
  - (pub struct) `pub struct BlockerInfo`
  - (pub enum) `pub enum GoalChangeKind`
  - (pub struct) `pub struct GoalDelta`

### `src/highlight.rs`
- uses:
  - `tower_lsp::lsp_types::{SemanticToken, SemanticTokenType, SemanticTokenModifier}`
  - `crate::document::ByteSpan`
  - `comrade_lisp::parser`
  - `comrade_lisp::syntax::{SExpr, SExprKind, Atom}`
- items:
  - (pub struct) `pub struct HighlightCtx<'a>`
  - (pub enum) `pub enum SymbolRole`
  - (priv impl) `impl SymbolRole`
  - (pub fn) `pub fn to_lsp_type(&self) -> SemanticTokenType`
  - (pub fn) `pub fn modifiers(&self) -> u32`
  - (pub const) `pub const LEGEND_TOKEN_TYPES: &[SemanticTokenType] = &[`
  - (pub const) `pub const LEGEND_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[`
  - (pub fn) `pub fn tokens_to_lsp_data(text: &str, tokens: &mut [(ByteSpan, SymbolRole)]) -> Vec<SemanticToken>` — Encodes internal tokens to LSP delta format (Absolute -> Relative). Must be sorted by Position.
  - (pub fn) `pub fn compute_layer0_structural(text: &str) -> Vec<(ByteSpan, SymbolRole)>` — Compute Layer 0 structural tokens.
  - (priv fn) `fn traverse_sexpr(expr: &SExpr, out: &mut Vec<(ByteSpan, SymbolRole)>)`
  - (priv fn) `fn handle_special_form_head(expr: &SExpr, role: SymbolRole, out: &mut Vec<(ByteSpan, SymbolRole)>)`
  - (priv fn) `fn scan_fallback(text: &str, out: &mut Vec<(ByteSpan, SymbolRole)>)` — Simple fallback scanner for Layer 0 when parser fails.

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
- uses:
  - `crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary, ReconstructionPlan}`
  - `super::LoogleResult`
  - `std::collections::HashMap`
  - `sha2::{Sha256, Digest}`
- items:
  - (pub struct) `pub struct ApplicabilityResult` — Applicability engine that checks if a lemma can unify with a goal Result of checking applicability of a lemma to a goal
  - (pub fn) `pub fn check_applicability(` — Check if a lemma result is applicable to a current goal
  - (priv enum) `enum TermStructure` — Lightweight structural representation for unification
  - (priv fn) `fn parse_fingerprint(fp: &str) -> TermStructure` — Parse a fingerprint string into a TermStructure
  - (priv fn) `fn unify_structures(` — Simple first-order unification
  - (priv fn) `fn unify_inner(` — Recursive unification helper
  - (priv fn) `fn compute_confidence(subst: &HashMap<String, TermStructure>) -> f32` — Compute confidence based on substitution complexity
  - (priv fn) `fn structure_complexity(s: &TermStructure) -> usize` — Estimate complexity of a structure
  - (priv fn) `fn format_substitutions(subst: &HashMap<String, TermStructure>) -> String` — Format substitutions for user display
  - (priv fn) `fn format_structure(s: &TermStructure) -> String` — Format a TermStructure for display
  - (pub fn) `pub fn to_proposal(` — Convert a LoogleResult with applicability check into a Proposal
  - (priv fn) `fn compute_proposal_id(anchor: &str, lemma_id: &str, confidence: f32) -> String` — Compute deterministic proposal ID via SHA256 content addressing Format: sha256("loogle:v1" || anchor || lemma_id || confidence)
  - (pub struct) `pub struct LemmaPayload`

### `src/loogle/code_actions.rs`
- uses:
  - `crate::loogle::{check_applicability, to_proposal, LoogleResult}`
  - `tower_lsp::lsp_types::{CodeAction, CodeActionKind, WorkspaceEdit, TextEdit, Range}`
  - `std::collections::HashMap`
  - `comrade_lisp::proof_state::ProofState`
- items:
  - (pub fn) `pub fn generate_loogle_actions(` — Loogle code actions for lemma suggestions Generate Loogle-based code actions for the current cursor position
  - (priv fn) `fn create_apply_lemma_action(`

### `src/loogle/context.rs`
- uses:
  - `super::LoogleResult`
  - `super::*`
- items:
  - (pub struct) `pub struct GoalContext` — Goal context module for cursor-position-aware search Provides relevance scoring for lemma suggestions based on the current proof context (goal fingerprint, surrounding bindings). Context information about the current goal being proved
  - (priv impl) `impl GoalContext`
  - (pub fn) `pub fn new(goal_fingerprint: String) -> Self` — Create a new goal context
  - (pub fn) `pub fn with_bindings(mut self, bindings: Vec<String>) -> Self` — Add surrounding bindings
  - (pub fn) `pub fn with_cursor(mut self, line: usize, col: usize) -> Self` — Add cursor position
  - (pub fn) `pub fn relevance_score(&self, lemma: &LoogleResult) -> f32` — Compute relevance score for a lemma in this context Returns a score between 0.0 and 1.0 indicating how relevant the lemma is to the current goal context.
  - (priv fn) `fn count_matching_bindings(&self, lemma: &LoogleResult) -> usize` — Count how many local bindings are mentioned in the lemma
  - (pub fn) `pub fn rank_results(&self, results: Vec<LoogleResult>) -> Vec<(LoogleResult, f32)>` — Filter and sort lemma results by relevance to this context
  - (priv fn) `fn fingerprint_similarity(fp1: &str, fp2: &str) -> f32` — Compute approximate similarity between two fingerprints Uses token overlap as a simple heuristic for structural similarity.
  - (priv fn) `fn test_goal_context_basic()`
  - (priv fn) `fn test_fingerprint_similarity_identical()`
  - (priv fn) `fn test_fingerprint_similarity_different()`

### `src/loogle/indexer.rs`
- uses:
  - `super::LoogleIndex`
  - `comrade_lisp::core::{CoreBundleV0, CompiledRule}`
  - `tcb_core::ast::MorphismTerm`
  - `tcb_core::ast::constructor_registry::TermArg`
  - `std::collections::hash_map::DefaultHasher`
  - `std::hash::{Hash, Hasher}`
- items:
  - (pub const) `pub const LOOGLE_FP_VERSION: u32 = 1;` — Fingerprint format version - increment when changing fingerprint structure to invalidate stale indexes and prevent version drift.
  - (priv const) `const MAX_FP_DEPTH: usize = 32;` — Maximum recursion depth for fingerprinting to prevent runaway on cyclic structures
  - (priv const) `const MAX_FP_NODES: usize = 256;` — Maximum number of nodes to fingerprint before truncation
  - (priv const) `const TRUNCATION_MARKER: &str = "…";` — Truncation marker for fingerprints that exceeded bounds
  - (pub struct) `pub struct WorkspaceIndexer` — Extracts and indexes lemmas from a workspace bundle
  - (priv impl) `impl WorkspaceIndexer`
  - (pub fn) `pub fn new() -> tantivy::Result<Self>`
  - (pub fn) `pub fn reindex(&self, bundle: &CoreBundleV0) -> tantivy::Result<()>` — Re-index the entire workspace from a new bundle
  - (pub fn) `pub fn index(&self) -> &LoogleIndex` — Get the underlying index for search operations
  - (priv fn) `fn is_lemma(rule: &CompiledRule) -> bool` — Check if a rule is marked as a lemma
  - (priv fn) `fn extract_lemma_name(rule: &CompiledRule) -> String` — Extract a human-readable name from the rule metadata or synthesize one
  - (pub fn) `pub fn compute_fingerprint(term: &tcb_core::ast::MorphismTerm) -> String` — Compute a structural fingerprint for search indexing This produces a canonical, deterministic string representation of the term structure that can be used for structural search. The format captures term shape while abstracting over specific IDs. Format: `v{VERSION}:{fingerprint}` **Invariant G6**: No Debug-derived strings used in fingerprints. All formatting uses stable, versioned accessors.
  - (priv struct) `struct FingerprintContext` — Fingerprinting context for tracking bounds
  - (priv impl) `impl FingerprintContext`
  - (priv fn) `fn new() -> Self`
  - (priv fn) `fn exceeded(&self) -> bool` — Check if we've exceeded bounds
  - (priv fn) `fn enter(&mut self) -> bool` — Enter a child node, returning true if we should process it
  - (priv fn) `fn exit(&mut self)` — Exit a child node
  - (priv fn) `fn truncate(&mut self) -> String` — Mark as truncated and return truncation marker
  - (priv fn) `fn compute_fingerprint_bounded(` — Bounded fingerprinting with depth/node tracking
  - (priv fn) `fn format_term_arg(arg: &tcb_core::ast::constructor_registry::TermArg) -> String` — Format a term argument for fingerprinting
  - (priv fn) `fn compute_hash(term: &tcb_core::ast::MorphismTerm) -> u64` — Compute stable hash for synthesized names
  - (priv fn) `fn extract_doc_string(rule: &CompiledRule) -> String` — Extract documentation string from rule metadata

### `src/loogle/mod.rs`
- modules:
  - `pub mod indexer;`
  - `pub mod applicability;`
  - `pub mod code_actions;`
  - `pub mod context;`
- uses:
  - `tantivy::schema::*`
  - `tantivy::{Index, IndexWriter, IndexReader, ReloadPolicy, doc}`
  - `tantivy::collector::TopDocs`
  - `tantivy::query::QueryParser`
  - `tantivy::TantivyDocument`
  - `std::sync::{Arc, RwLock}`
- items:
  - (pub struct) `pub struct LoogleIndex`
  - (priv impl) `impl LoogleIndex`
  - (pub fn) `pub fn new_in_memory() -> tantivy::Result<Self>`
  - (pub fn) `pub fn index_lemma(` — Index a lemma from the workspace bundle
  - (pub fn) `pub fn search(&self, query_fp: &str, limit: usize) -> tantivy::Result<Vec<LoogleResult>>` — Search for lemmas by structural fingerprint
  - (pub fn) `pub fn clear(&self) -> tantivy::Result<()>` — Clear all indexed lemmas (for re-indexing)
  - (pub struct) `pub struct LoogleResult`

### `src/lsp.rs`
- uses:
  - `std::{collections::BTreeMap, sync::Arc, time::Duration}`
  - `serde::{Deserialize, Serialize}`
  - `source_span::Span`
  - `tokio::sync::{mpsc, RwLock}`
  - `tokio::process::Command`
  - `tokio::time::{self, Instant}`
  - `crate::span_conversion::{byte_span_to_lsp_range, span_to_lsp_range}`
  - `crate::proof_session::{ProofSession, ProofSessionOpenResult, ProofSessionUpdateResult}`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `comrade_lisp::WorkspaceDiagnostic`
  - `comrade_lisp::{MadLibResolver}`
  - `comrade_lisp::diagnostics::pretty::{PrettyDialect, PrettyLimits}`
  - `comrade_lisp::diagnostics::{StructuredDiagnostic, DiagnosticOrigin, DiagnosticContext, canonical_diag_sort_key}`
  - `comrade_lisp::scopecreep::{run_scopecreep, ScopeCreepOptions, ScopeCreepInput}`
  - `sniper_db::SniperDatabase`
  - `sniper_db::diagnostic::DiagnosticSeverity as SniperSeverity`
  - `sniper_db::diagnostic::RelatedInfoKind`
  - `async_trait::async_trait`
  - `super::*`
  - `crate::document::ParsedDocument`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `comrade_lisp::{WorkspaceDiagnostic, WorkspaceDiagnosticSeverity}`
- items:
  - (priv const) `const EXTERNAL_COMMAND_TIMEOUT_MS: u64 = 5000;`
  - (priv const) `const DEFAULT_DEBOUNCE_INTERVAL_MS: u64 = 250;`
  - (priv const) `const DEFAULT_LOG_LEVEL: &str = "info";`
  - (priv const) `const DEFAULT_EXTERNAL_COMMAND: Option<&str> = None;`
  - (priv fn) `fn is_identifier_char(c: char) -> bool`
  - (priv fn) `fn symbol_at_position(text: &str, position: Position) -> Option<String>`
  - (priv fn) `fn render_hover_markdown(from: &str, to: &str, report: &sniper_db::plan::PlanReport, debug_mode: bool) -> String`
  - (pub struct) `pub struct DebouncedDocumentChange`
  - (pub struct) `pub struct Config`
  - (priv fn) `fn default_caches_enabled() -> bool`
  - (priv impl) `impl Default for Config`
  - (priv fn) `fn default() -> Self`
  - (priv fn) `fn parse_external_output_to_diagnostics(_uri: &Url, output_line: &str) -> Diagnostic`
  - (priv fn) `async fn run_external_command_and_parse_diagnostics(`
  - (pub fn) `pub fn document_diagnostics_from_report(`
  - (priv fn) `fn clamp_span(span: Span, text_len: usize) -> Span`
  - (priv fn) `fn byte_span_to_range(text: &str, span: ByteSpan) -> Range`
  - (priv fn) `fn convert_sniper_diagnostic_to_lsp(` — Convert SniperDB diagnostic to LSP diagnostic
  - (priv fn) `fn chain_to_selection_range(text: &str, chain: &[ByteSpan]) -> SelectionRange`
  - (pub fn) `pub fn workspace_error_report(err: &SurfaceError) -> WorkspaceReport`
  - (pub struct) `pub struct PublishDiagnosticsHandler;` — PublishDiagnosticsHandler - Centralized diagnostic publishing system This handler is responsible for: - Converting SniperDB diagnostics to LSP format - Using the span conversion system for precise UTF-16 positions - Sorting diagnostics deterministically # CHOKE POINT ENFORCEMENT This is the ONLY code path that publishes diagnostics to LSP. All diagnostic publication must flow through this handler. This is structurally enforced: 1. `publish_diagnostics_canonical()` is the single entry point 2. All other methods are private or internal 3. No other code can call `client.publish_diagnostics()` directly 4. Attempting to bypass this will fail at compile time # Requirements - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention) - Validates: Requirements 5.1, 5.3 (LSP Integration)
  - (priv impl) `impl PublishDiagnosticsHandler`
  - (pub fn) `pub async fn publish_diagnostics_canonical(` — CANONICAL CHOKE POINT: Publish diagnostics for a document This is the ONLY function that publishes diagnostics to LSP. All diagnostic publication must flow through this function. This function: 1. Converts diagnostics from internal format to LSP format 2. Sorts diagnostics deterministically 3. Publishes all diagnostics through LSP protocol # Arguments - `client`: LSP client for publishing - `uri`: Document URI - `report`: WorkspaceReport containing all diagnostics - `parsed_doc`: Parsed document for span conversion - `version`: Optional document version # Requirements - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention) - Validates: Requirements 5.1, 5.3 (LSP Integration)
  - (pub fn) `pub async fn publish_diagnostics_canonical_preconverted(` — CANONICAL CHOKE POINT: Publish pre-converted diagnostics This is the ONLY function that publishes pre-converted diagnostics to LSP. Use this when diagnostics have already been converted and sorted. # Arguments - `client`: LSP client for publishing - `uri`: Document URI - `diagnostics`: Pre-converted and sorted diagnostics - `version`: Optional document version # Requirements - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention) - Validates: Requirements 5.1, 5.3 (LSP Integration)
  - (pub fn) `pub async fn publish_diagnostics(`
  - (pub fn) `pub async fn publish_preconverted(`
  - (priv fn) `async fn publish_diagnostics_internal(` — INTERNAL: The actual LSP publication point This is the single point where diagnostics are published to LSP. This function is private to prevent bypass. # Requirements - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention)
  - (pub fn) `pub fn convert_diagnostics(` — **INV D-PUBLISH-CORE**: Extract Core-only diagnostics from report (Phase 1). Convert diagnostics from internal format to LSP format This function: 1. Converts parser diagnostics 2. Converts workspace report diagnostics 3. Sorts all diagnostics deterministically # Requirements - Validates: Requirements 5.1, 5.3
  - (priv fn) `fn convert_parser_diagnostic(text: &str, pd: &crate::document::ParseDiagnostic) -> Diagnostic` — Convert a single parser diagnostic to LSP format
  - (priv fn) `fn convert_workspace_diagnostics(` — Convert workspace report diagnostics to LSP format Uses the span conversion system to ensure UTF-16 correctness. Prioritizes structured_diagnostics (from Task 15 multi-diagnostic collection) over legacy diagnostics for backward compatibility. # Requirements - Validates: Requirements 5.1, 5.2, 6.1, 6.2, 8.3, 8.5
  - (priv fn) `fn convert_severity(severity: WorkspaceDiagnosticSeverity) -> DiagnosticSeverity` — Convert workspace diagnostic severity to LSP severity
  - (pub fn) `pub fn sort_diagnostics(uri: &Url, diagnostics: &mut Vec<Diagnostic>)` — Sort diagnostics deterministically Diagnostics are sorted by: 1. URI 2. Reliability (spanned diagnostics before spanless) 3. Severity (errors before warnings before info) 4. Position (line, then character) 5. Message 6. Code 7. Source # Requirements - Validates: Requirements 5.3
  - (priv fn) `fn diagnostic_sort_key<'a>(` — Generate sort key for a diagnostic
  - (priv fn) `fn severity_rank(severity: Option<DiagnosticSeverity>) -> u8` — Convert diagnostic severity to rank for sorting
  - (priv fn) `fn diagnostic_code_to_str(code: Option<&NumberOrString>) -> String` — Convert diagnostic code to string for sorting
  - (priv fn) `fn diagnostic_source_to_str(source: Option<&String>) -> &str` — Convert diagnostic source to string for sorting
  - (pub struct) `pub struct Backend`
  - (priv impl) `impl Backend`
  - (pub fn) `pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type`
  - (priv fn) `async fn hover_db7_preview(&self, uri: &Url, position: Position) -> Option<Hover>` — DB-7 hover preview: show rename impact analysis
  - (priv fn) `async fn code_action_db7_preview(&self, uri: &Url, position: Position) -> Option<tower_lsp::lsp_types::CodeAction>` — DB-7 code action: offer "Preview Rename Impact" action
  - (priv fn) `async fn code_action_db7_preview_detailed(&self, uri: &Url, position: Position) -> Option<tower_lsp::lsp_types::CodeAction>` — DB-7 code action (detailed): offer "Preview Rename Impact (Detailed)" action
  - (priv fn) `async fn code_action_db7_preview_internal(` — Internal helper for DB-7 code actions
  - (priv fn) `async fn process_debounced_proof_session_events(`
  - (priv impl) `impl LanguageServer for Backend`
  - (priv fn) `async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult>`
  - (priv fn) `async fn initialized(&self, _: InitializedParams)`
  - (priv fn) `async fn shutdown(&self) -> Result<()>`
  - (priv fn) `async fn did_open(&self, params: DidOpenTextDocumentParams)`
  - (priv fn) `async fn did_change(&self, params: DidChangeTextDocumentParams)`
  - (priv fn) `async fn did_save(&self, params: DidSaveTextDocumentParams)`
  - (priv fn) `async fn did_close(&self, params: DidCloseTextDocumentParams)`
  - (priv fn) `async fn hover(&self, params: HoverParams) -> Result<Option<Hover>>`
  - (priv fn) `async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>>`
  - (priv fn) `async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>>`
  - (priv fn) `async fn selection_range(`
  - (priv fn) `async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>>`
  - (priv fn) `async fn semantic_tokens_full(`
  - (priv fn) `async fn document_symbol(`
  - (priv fn) `async fn diagnostic(`
  - (priv fn) `fn extract_trace_steps(_term: &str) -> Option<Vec<String>>` — Stub implementation for trace step extraction (TODO: implement)
  - (priv fn) `fn sample_report() -> WorkspaceReport`
  - (priv fn) `fn workspace_report_to_diagnostics_is_deterministic()`
  - (priv fn) `fn document_diagnostics_include_workspace_report_diag()`
  - (priv fn) `fn funnel_invariant_no_shadow_publish_calls()` — Funnel Invariant Test: Verify no direct publish_diagnostics calls outside handler This test ensures the architecture maintains a single publication funnel. Only the handler methods and did_close (for clearing) should call client.publish_diagnostics. Allowlist: - PublishDiagnosticsHandler::publish_diagnostics (line ~390) - PublishDiagnosticsHandler::publish_preconverted (line ~407) - did_close clearing call (line ~1060)
  - (priv fn) `fn test_extract_trace_steps_v1()`
  - (priv fn) `async fn test_extract_trace_steps_legacy()`
  - (priv fn) `fn test_extract_trace_steps_no_match()`

### `src/main.rs`
- uses:
  - `edgelord_lsp::{Backend, lsp::Config}`
  - `tower_lsp::{LspService, Server}`
  - `tokio::sync::RwLock`
  - `std::sync::Arc`
- items:
  - (priv fn) `async fn main()`

### `src/proof_session.rs`
- uses:
  - `std::{collections::{BTreeMap, VecDeque}, sync::Arc, time::Instant as StdInstant}`
  - `tokio::sync::RwLock`
  - `tokio::time::Instant`
  - `crate::document::{apply_content_changes, ParsedDocument, Goal}`
  - `crate::lsp::{Config, workspace_error_report, document_diagnostics_from_report}`
  - `crate::caching::{ModuleCache, CacheKey, CacheValue, CacheKeyBuilder, ModuleSnapshotCache, Phase1MissReason, Phase1_1MissReason, CacheOutcome}`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `comrade_lisp::ComradeWorkspace`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `comrade_lisp::diagnostics::DiagnosticContext`
  - `comrade_lisp::proof_state`
  - `comrade_lisp::diagnostics`
  - `comrade_lisp::ContentChange`
  - `codeswitch::fingerprint::HashValue`
  - `sniper_db::SniperDatabase`
  - `hex`
  - `crate::caching::ModuleSnapshot`
  - `comrade_lisp::diagnostics::ByteSpan`
  - `super::*`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `comrade_lisp::proof_state::{ProofState, MetaSubst, ElaborationTrace, GoalState, HoleOwner, MorMetaId, LocalContext, MorType, ObjExpr, GoalStatus}`
  - `comrade_lisp::diagnostics::{DiagnosticContext, ByteSpan}`
  - `tower_lsp::lsp_types::Url`
- items:
  - (pub struct) `pub struct BenchmarkMeasurement` — C2.4: Benchmark measurement record (19 CSV fields)
  - (priv impl) `impl BenchmarkMeasurement`
  - (pub fn) `pub fn to_csv_row(&self) -> String` — Write as CSV row (exact field order per spec)
  - (pub fn) `pub fn csv_header() -> &'static str` — CSV header row
  - (pub struct) `pub struct ProofSnapshot`
  - (pub struct) `pub struct ProofDocument`
  - (pub struct) `pub struct ProofSessionOpenResult`
  - (pub struct) `pub struct ProofSessionUpdateResult`
  - (pub struct) `pub struct ProofSession`
  - (priv impl) `impl ProofSession`
  - (pub fn) `pub fn new(client: Client, config: Arc<RwLock<Config>>, db: Arc<SniperDatabase>) -> Self { // Changed client type`
  - (pub fn) `pub fn get_document_version(&self, uri: &Url) -> Option<i32>` — Get the current document version for the given URI. **INV T-DVCMP Support**: Used by async ScopeCreep tasks to detect stale results (when document version has changed since Phase 1).
  - (pub fn) `pub async fn open(&mut self, uri: Url, version: i32, initial_text: String) -> ProofSessionOpenResult`
  - (pub fn) `pub async fn update(&mut self, uri: Url, version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> ProofSessionUpdateResult`
  - (pub fn) `pub fn get_document(&self, uri: &Url) -> Option<&ProofDocument>`
  - (pub fn) `pub fn get_goals(&self, uri: &Url) -> Vec<Goal>`
  - (pub fn) `pub fn loogle_index(&self) -> &crate::loogle::WorkspaceIndexer`
  - (pub fn) `pub async fn apply_command(&mut self, uri: Url, _command: String) -> ProofSessionUpdateResult`
  - (pub fn) `pub fn get_document_text(&self, uri: &Url) -> Option<String>`
  - (pub fn) `pub fn get_last_analyzed_time(&self, uri: &Url) -> Option<Instant>`
  - (pub fn) `pub fn get_parsed_document(&self, uri: &Url) -> Option<&ParsedDocument>`
  - (pub fn) `pub fn get_proof_state(&self, uri: &Url) -> Option<&proof_state::ProofState>`
  - (pub fn) `pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic>`
  - (pub fn) `pub fn resolve_goal_anchor(&self, uri: &Url, anchor_id: &str) -> Option<(proof_state::MorMetaId, Option<diagnostics::ByteSpan>)>`
  - (pub fn) `pub fn close(&mut self, uri: &Url)`
  - (pub fn) `pub fn compute_goals_panel(&self, uri: &Url) -> Option<crate::goals_panel::GoalsPanelResponse>`
  - (priv fn) `fn compute_ui_goals(`
  - (priv fn) `fn test_compute_ui_goals_stability()`
  - (priv fn) `fn test_caches_disabled_via_env_var()`
  - (priv fn) `fn compute_structural_summary(goal: &proof_state::GoalState) -> String`
  - (priv fn) `fn compute_workspace_snapshot_hash(` — Phase 1.1: Compute workspace snapshot hash from all open documents. This provides a conservative fingerprint that changes when any document changes. INV D-CACHE-3: Monotone invalidation (changes to any doc invalidate all caches)
  - (priv fn) `fn uri_to_file_id(uri: &Url) -> u32` — Phase 1: Convert URI to file ID for module snapshot cache. Uses CRC32 hash of URI string for deterministic, stable file identification.
  - (pub fn) `pub fn compute_options_fingerprint(config: &Config) -> HashValue` — Phase 1: Compute options fingerprint from canonical compile options. CRITICAL: Must include all options that affect compilation output. This must be updated whenever a new Config field is added that affects semantics. Current semantic-affecting fields: - pretty_dialect: Output formatting (Pythonic vs Canonical) - enable_db7_hover_preview: DB-7 feature enabled/disabled - db7_placeholder_suffix: Refactoring hint template - db7_debug_mode: Diagnostic detail level - external_command: External tool integration (if present) NOT included (timing/logging only): - debounce_interval_ms: Timing only, not output semantics - log_level: Logging only, not output semantics INV: Same options → same fingerprint (no false misses) Different options → different fingerprint (no false hits) TODO: When adding new Config fields, update this function and add a test case.
  - (priv fn) `fn compute_dependency_fingerprint_conservative(` — Phase 1: Compute dependency fingerprint (conservative: workspace snapshot). For Phase 1, we use the workspace snapshot hash as the dependency fingerprint. This is conservative but sound: changes to ANY document invalidate all caches. TODO Phase 1.2: Implement true transitive dependency tracking - Build actual import graph - Only invalidate units affected by changed imports - Reduce false misses from unrelated document changes INV: Same workspace state → same fingerprint Different imports → cache miss (conservative but correct)
  - (priv fn) `fn normalize_unit_id(uri: &Url) -> String` — Phase 1.1: Normalize unit identifier to canonical form. As per PHASE_1_1_ACCEPTANCE_SPEC.md §1.1: - Prefer FileId if available - Otherwise normalize Url string: no fragment/query, forward slashes, preserve case
  - (priv fn) `fn hash_to_fp8(hash: &HashValue) -> String` — Convert HashValue to 8-char hex string (first 8 bytes)
  - (priv fn) `fn bytes_open_docs(documents: &BTreeMap<Url, ProofDocument>) -> usize` — Get total bytes of all open documents in deterministic order

### `src/proposal.rs`
- uses:
  - `serde::{Serialize, Deserialize}`
- items:
  - (pub struct) `pub struct Proposal<T>` — A universal protocol for knowledge-sharing suggestions.
  - (pub enum) `pub enum ProposalKind`
  - (pub enum) `pub enum ProposalStatus`
  - (pub struct) `pub struct EvidenceSummary`
  - (pub struct) `pub struct ReconstructionPlan`

### `src/queries/check_unit.rs`
- uses:
  - `std::collections::BTreeMap`
  - `codeswitch::fingerprint::HashValue`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `tower_lsp::lsp_types::Diagnostic`
  - `super::*`
- items:
  - (pub struct) `pub struct CompileInputV1` — Phase 1.2B: DB-Native Compile Query - Q_CHECK_UNIT_V1 This query captures the canonical inputs for unit compilation and stores the results in SniperDB as the sole source of truth for incremental reuse. Hard Invariants: - **Purity**: Same input → same output, deterministically - **Sound Reuse**: Output valid only when input hash matches exactly - **Persistence**: Results survive restarts and process boundaries - **Single-flight**: Compilation happens at most once per unique input - **Stonewall**: No side effects (no workspace mutations during memo lookup) Phase 1.2B: Canonical compilation input for unit check query All fields are deterministically serialized to create a stable input hash. No hidden non-determinism (e.g., no paths, no timestamps, no Debug strings). Components: 1. **unit_content**: Source code bytes 2. **compile_options**: Pretty printer dialect, feature flags, etc. 3. **workspace_snapshot**: All open documents (conservative dependency model) 4. **file_id**: Stable file identifier (CRC32 of URI) Serialization: Canonical byte ordering (sorted collections, explicit separators)
  - (priv impl) `impl CompileInputV1`
  - (pub fn) `pub fn compute_digest(` — Compute input digest from canonical serialization
  - (pub fn) `pub fn new(` — Create a new CompileInputV1 with computed digest
  - (pub struct) `pub struct Q_CHECK_UNIT_V1;` — Phase 1.2B: Named query for unit compilation Query: Q_CHECK_UNIT_V1 Input: CompileInputV1 (deterministically serialized) Output: DiagnosticsArtifactV1 (compilation results) Guarantee: For any given input_digest, always returns the same output. Storage: Results persisted in SniperDB's memo table by input_digest.
  - (priv impl) `impl Q_CHECK_UNIT_V1`
  - (pub const) `pub const NAME: &'static str = "Q_CHECK_UNIT_V1";`
  - (pub fn) `pub fn name() -> &'static str` — Canonical query name for logging and introspection
  - (pub fn) `pub fn query_class() -> &'static str` — Query class (e.g., "incremental_check", "unit_compile")
  - (pub fn) `pub fn input_version() -> u32` — Expected input type version
  - (pub fn) `pub fn output_version() -> u32` — Expected output type version
  - (pub struct) `pub struct DiagnosticsArtifactV1` — Phase 1.2B: Compilation output artifact for Q_CHECK_UNIT_V1 Captures the canonical output of unit compilation: - WorkspaceReport: Type information, proof state, diagnostics - Computed diagnostics: Projected to LSP format - Timestamp: When this artifact was computed Guarantee: Deterministic given the input. No side effects or mutations.
  - (priv impl) `impl DiagnosticsArtifactV1`
  - (pub fn) `pub fn new(` — Create a new diagnostics artifact with optional soundness proof
  - (pub fn) `pub fn verify_determinism(&self, expected_digest: &HashValue) -> bool` — Verify that output is deterministic (optional: compare with expected digest)
  - (priv fn) `fn test_compile_input_v1_digest_determinism()`
  - (priv fn) `fn test_compile_input_v1_digest_changes_with_content()`
  - (priv fn) `fn test_compile_input_v1_digest_changes_with_options()`
  - (priv fn) `fn test_q_check_unit_v1_constants()`

### `src/queries/mod.rs`
- modules:
  - `pub mod check_unit;`

### `src/refute/lsp_handler.rs`

LSP handler for edgelord/refute endpoint.

This module provides the stable JSON contract for refutation.

- uses:
  - `serde::{Deserialize, Serialize}`
  - `sha2::{Sha256, Digest}`
  - `crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary}`
  - `crate::refute::types::{BoundedList, RefuteLimits, StableAnchor, AnchorKind, DecisionInfo}`
  - `crate::refute::witness::{CounterexamplePayload, Counterexample, FailureWitness, InterpretationSummary, InterpretationKind, DiagramBoundary}`
  - `crate::refute::probe::ProbeKey`
  - `crate::refute::slice::SliceSummary`
  - `std::time::{SystemTime, UNIX_EPOCH}`
  - `super::*`
- items:
  - (pub const) `pub const REFUTE_PROTOCOL_VERSION: u32 = 1;` — Version of the refute protocol. Bump on breaking changes.
  - (pub struct) `pub struct RefuteRequest` — Request for refutation.
  - (pub struct) `pub struct RefuteResponse` — Response envelope for refutation. **Invariant**: Field names and ordering are stable. Bump `version` on breaking changes.
  - (pub struct) `pub struct RefuteMeta` — Metadata about the refutation request.
  - (pub fn) `pub fn sort_proposals(proposals: &mut [Proposal<CounterexamplePayload>])` — Sort proposals deterministically. Order by: status, level, probe, score (desc), failure fingerprint, id
  - (priv fn) `fn status_rank(status: &ProposalStatus) -> u8`
  - (priv fn) `fn failure_fingerprint(witness: &FailureWitness) -> String` — Compute fingerprint for a failure witness.
  - (pub fn) `pub fn generate_proposal_id(anchor: &str, probe: &ProbeKey, witness: &FailureWitness) -> String` — Generate deterministic proposal ID from (anchor, probe, witness fingerprint).
  - (pub fn) `pub fn handle_refute_request(` — Handle a refute request and return a stable response. **Invariant**: Output is byte-for-byte deterministic for same input.
  - (priv fn) `fn create_missing_coherence_proposal(` — Create a MissingCoherenceWitness proposal for level ≥ 1.
  - (priv fn) `fn current_timestamp_ms() -> u64`
  - (priv fn) `fn round_score(score: f32) -> f32`
  - (priv fn) `fn test_generate_proposal_id_deterministic()`
  - (priv fn) `fn test_handle_refute_deterministic()`
  - (priv fn) `fn test_coherence_level_1_returns_missing_witness()`

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

- uses:
  - `std::sync::Arc`
  - `crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary}`
  - `crate::refute::types::{RefuteLimits, StableAnchor, DecisionInfo}`
  - `crate::refute::probe::{ProbeDoctrine, ProbeKey, RefuteCheckResult, InterpretationData}`
  - `crate::refute::slice::{RefuteSlice, SliceSummary}`
  - `crate::refute::witness::{CounterexamplePayload, Counterexample, InterpretationSummary, InterpretationKind, FailureWitness}`
  - `serde::{Deserialize, Serialize}`
  - `sha2::{Sha256, Digest}`
  - `crate::refute::probes::rewrite_probe::RewriteProbe`
- items:
  - (pub struct) `pub struct RefuteRequest` — Request for refutation.
  - (pub struct) `pub struct Refuter` — Main refuter orchestrator. **Invariants**: - Deterministic probe ordering - Returns Proposal<CounterexamplePayload>, not bespoke result - ProposalStatus::Advisory always for Phase 10
  - (priv impl) `impl Refuter`
  - (pub fn) `pub fn new(probes: Vec<Arc<dyn ProbeDoctrine>>) -> Self` — Create a new refuter with the given probes.
  - (pub fn) `pub fn with_default_probes() -> Self` — Create a refuter with default MVP probes.
  - (pub fn) `pub fn refute(` — Run refutation and return a Proposal. **Semantics**: - Tries probes in deterministic order - Returns first counterexample found - Returns "no counterexample within bounds" if nothing found
  - (priv fn) `fn probes_in_order<'a>(` — Get probes in deterministic order for this slice.
  - (priv fn) `fn make_proposal(` — Create a proposal from a counterexample.
  - (priv fn) `fn make_no_counterexample_proposal(` — Create a "no counterexample found" proposal.
  - (priv fn) `fn compute_proposal_id(` — Compute deterministic proposal ID.

### `src/refute/probe.rs`

Probe trait and core probe types.

A probe doctrine is a small semantic world we can map into and evaluate.
This is the Mac Lane-native approach: interpretations are first-class objects.

- uses:
  - `serde::{Deserialize, Serialize}`
  - `crate::refute::types::{BoundedList, RefuteLimits, RefuteFragment, TruncationReason}`
  - `crate::refute::slice::RefuteSlice`
  - `crate::refute::witness::FailureWitness`
- items:
  - (pub struct) `pub struct ProbeKey` — Stable identity for a probe doctrine. **Mac Lane move**: includes deformation level and semantic description for education + ML training data.
  - (pub struct) `pub struct InterpretationCandidate` — A candidate interpretation from a probe doctrine. Stored structurally; rendered via PrettyCtx at the boundary.
  - (pub enum) `pub enum InterpretationData` — Probe-specific interpretation data.
  - (pub enum) `pub enum RefuteCheckResult` — Result of checking an interpretation. **Critical invariant**: `NoFailureFoundWithinBounds` is NOT a counterexample. Only `FoundFailure` with a witness counts as refutation.
  - (pub trait) `pub trait ProbeDoctrine: Send + Sync` — A probe doctrine for Mac Lane-native refutation. Probes enumerate interpretations and check whether obligations hold. This is doctrine-agnostic: the same interface works for rewrite probes, finite category probes, and (eventually) SMT probes.
  - (priv fn) `fn key(&self) -> ProbeKey;` — Stable identity for this probe.
  - (priv fn) `fn max_level(&self) -> u8;` — Maximum coherence level this probe supports.
  - (priv fn) `fn supports_fragment(&self, frag: &RefuteFragment) -> bool;` — Whether this probe can handle the given fragment.
  - (priv fn) `fn enumerate_interpretations(` — Enumerate candidate interpretations (bounded, deterministic).
  - (priv fn) `fn check(` — Check whether obligations hold under this interpretation. **Invariant**: Must return `NoFailureFoundWithinBounds` on timeout, not `FoundFailure`. Only genuine obstructions yield `FoundFailure`.

### `src/refute/probes/finite_cat_probe.rs`

Finite skeletal category probe (MVP Probe B).

Checks obligations in small finite categories (N≤3 default).
Returns UnsupportedFragment if categorical presentation not extractable.

- uses:
  - `crate::refute::types::{BoundedList, RefuteLimits, RefuteFragment, TruncationReason}`
  - `crate::refute::slice::RefuteSlice`
  - `super::*`
  - `crate::refute::types::{StableAnchor, AnchorKind}`
- items:
  - (pub struct) `pub struct FiniteCatProbe;` — Finite skeletal category probe. **Semantics**: - Domain: objects 0..N-1 - Morphisms: generated by slice with explicit composition - Checks: composition, identities, associativity - Returns `UnsupportedFragment` if categorical presentation not extractable
  - (priv impl) `impl FiniteCatProbe`
  - (pub fn) `pub fn new() -> Self`
  - (priv impl) `impl ProbeDoctrine for FiniteCatProbe`
  - (priv fn) `fn key(&self) -> ProbeKey`
  - (priv fn) `fn max_level(&self) -> u8`
  - (priv fn) `fn supports_fragment(&self, frag: &RefuteFragment) -> bool`
  - (priv fn) `fn enumerate_interpretations(`
  - (priv fn) `fn check(`
  - (priv impl) `impl Default for FiniteCatProbe`
  - (priv fn) `fn default() -> Self`
  - (priv fn) `fn test_finite_cat_probe_key()`
  - (priv fn) `fn test_finite_cat_probe_supports_categorical()`
  - (priv fn) `fn test_enumerate_respects_limits()`

### `src/refute/probes/mod.rs`

Probe implementations submodule.

- modules:
  - `pub mod rewrite_probe;`
  - `pub mod finite_cat_probe;`

### `src/refute/probes/rewrite_probe.rs`

Rewrite obstruction probe (MVP Probe A).

Detects failures via rewriting: non-joinable peaks, critical pairs.
Returns honest results: only FoundFailure when genuine obstruction found.

- uses:
  - `crate::refute::types::{BoundedList, RefuteLimits, RefuteFragment, TruncationReason}`
  - `crate::refute::slice::{RefuteSlice, Obligation}`
  - `crate::refute::witness::FailureWitness`
  - `super::*`
  - `crate::refute::slice::TermRef`
  - `crate::refute::types::{StableAnchor, AnchorKind}`
- items:
  - (pub struct) `pub struct RewriteProbe;` — Rewrite obstruction probe. **Semantics**: - Treats slice as a rewriting system - Searches for non-joinable peaks / critical pairs - Returns `NoFailureFoundWithinBounds` on timeout (NOT a false refutation)
  - (priv impl) `impl RewriteProbe`
  - (pub fn) `pub fn new() -> Self`
  - (priv impl) `impl ProbeDoctrine for RewriteProbe`
  - (priv fn) `fn key(&self) -> ProbeKey`
  - (priv fn) `fn max_level(&self) -> u8`
  - (priv fn) `fn supports_fragment(&self, frag: &RefuteFragment) -> bool`
  - (priv fn) `fn enumerate_interpretations(`
  - (priv fn) `fn check(`
  - (priv impl) `impl Default for RewriteProbe`
  - (priv fn) `fn default() -> Self`
  - (priv fn) `fn test_slice_with_obligations(obligations: Vec<Obligation>) -> RefuteSlice`
  - (priv fn) `fn test_rewrite_probe_key()`
  - (priv fn) `fn test_rewrite_probe_supports_equational()`
  - (priv fn) `fn test_rewrite_probe_finds_nonjoinable_peak()`
  - (priv fn) `fn test_rewrite_probe_no_failure_for_equal()`

### `src/refute/render.rs`

Witness rendering (MVVM separation).

Keeps witnesses structural; renders to pretty text at the boundary.

- uses:
  - `crate::refute::witness::{Counterexample, FailureWitness}`
  - `super::*`
  - `crate::refute::probe::ProbeKey`
  - `crate::refute::slice::SliceSummary`
  - `crate::refute::types::{BoundedList, JumpTarget}`
- items:
  - (pub fn) `pub fn render_counterexample(cx: &Counterexample) -> String` — Render a counterexample to human-readable text.
  - (pub fn) `pub fn render_failure_witness(witness: &FailureWitness) -> String` — Render a failure witness to human-readable text.
  - (pub struct) `pub struct RefuteExplanationNode` — A simplified ExplanationNode for refutation witnesses. This mirrors the structure in `explain::view` but is standalone for the refute module to avoid circular dependencies.
  - (priv impl) `impl RefuteExplanationNode`
  - (pub fn) `pub fn leaf(label: String, kind: &str) -> Self`
  - (pub fn) `pub fn with_jump(mut self, start: usize, end: usize) -> Self`
  - (pub fn) `pub fn witness_to_explanation_tree(witness: &FailureWitness) -> RefuteExplanationNode` — Convert a failure witness to an explanation tree. **Invariant**: All labels PrettyCtx-rendered, jump targets byte-offset. UTF-16 conversion happens at LSP boundary via `byte_to_utf16_offset`.
  - (pub fn) `pub fn validate_tree_spans(node: &RefuteExplanationNode, text_len: usize) -> bool` — Validate that all jump targets in a tree are within bounds. Returns `true` if all spans are valid.
  - (priv fn) `fn test_render_equation_failure()`
  - (priv fn) `fn test_witness_to_explanation_tree()`
  - (priv fn) `fn test_utf16_jump_target_validity()`

### `src/refute/slice.rs`

Refutation slice extraction.

A RefuteSlice is a minimal, trace-driven theory slice containing:
- Target obligation(s)
- Minimal blocker set
- Bounded neighborhood of rules/defs from trace graph

- uses:
  - `serde::{Deserialize, Serialize}`
  - `crate::refute::types::{BoundedList, RefuteLimits, TruncationReason, StableAnchor, AnchorKind}`
  - `super::*`
- items:
  - (pub struct) `pub struct RefuteSlice` — A minimal theory slice for refutation. **Key invariants**: - `anchor` is structured (not String) - Obligations sorted deterministically - Rules/defs bounded by trace neighborhood
  - (pub struct) `pub struct Obligation` — An obligation to check in the refutation.
  - (pub struct) `pub struct TermRef` — Reference to a term (structural, rendered via PrettyCtx).
  - (pub struct) `pub struct RuleRef` — Reference to a rule in the slice.
  - (pub struct) `pub struct DefRef` — Reference to a definition in the slice.
  - (pub struct) `pub struct CoherenceObligation` — A coherence obligation for level≥1 proofs. When coherence level > 0, the slice includes diagram boundaries that need to be checked or surfaced as "missing coherence" if undecidable.
  - (pub struct) `pub struct DiagramBoundary` — A diagram boundary requiring a higher cell.
  - (pub struct) `pub struct SliceSummary` — Summary of what was included in the slice. Explicitly educational: shows what was assumed for the refutation.
  - (priv impl) `impl SliceSummary`
  - (pub fn) `pub fn from_slice(slice: &RefuteSlice, trace_steps: usize) -> Self` — Create summary from a slice.
  - (pub fn) `pub fn extract_slice(` — Extract a minimal slice for refutation. **Policy**: - Start from target goal/constraint - Include minimal blockers (from GoalsIndex) - Include rules/defs from trace neighborhood up to max_trace_steps - Deterministic sort everywhere
  - (priv fn) `fn test_anchor() -> StableAnchor`
  - (priv fn) `fn test_extract_slice_empty()`

### `src/refute/types.rs`

Core types for Mac Lane-native refutation engine.

These types follow the universal bounded list schema and use structured
internal representations (no pre-rendered strings).

- uses:
  - `serde::{Deserialize, Serialize}`
  - `super::*`
- items:
  - (pub struct) `pub struct StableAnchor` — A stable identifier for a proof artifact. This is a local copy to avoid deep dependency paths. Matches the structure in `new_surface_syntax::diagnostics::anchors`.
  - (pub enum) `pub enum AnchorKind`
  - (priv impl) `impl StableAnchor`
  - (pub fn) `pub fn to_id_string(&self) -> String` — Compute the deterministic string ID.
  - (pub fn) `pub fn test(kind: AnchorKind, file_uri: &str, owner_path: Vec<String>, ordinal: u32, span_fingerprint: u64) -> Self` — Create a test anchor.
  - (pub struct) `pub struct BoundedList<T>` — A bounded list with full truncation tracking. **Invariant**: This is the universal bounded list type used across EdgeLorD (refute, explain, loogle). Always includes: - `total_count`: how many items existed before capping - `truncation_reason`: why we stopped (if truncated)
  - (pub fn) `pub fn from_vec(items: Vec<T>) -> Self` — Create a non-truncated list.
  - (pub fn) `pub fn truncated(items: Vec<T>, total_count: usize, reason: TruncationReason) -> Self` — Create a truncated list.
  - (pub fn) `pub fn empty() -> Self` — Create an empty list.
  - (priv fn) `fn default() -> Self`
  - (pub enum) `pub enum TruncationReason` — Reason for truncation in bounded operations.
  - (pub struct) `pub struct DecisionInfo` — Uniform decision envelope for every witness. This prevents clients from having to interpret enum variants as policy.
  - (priv impl) `impl DecisionInfo`
  - (pub fn) `pub fn decided() -> Self` — Decided failure within decidable fragment.
  - (pub fn) `pub fn not_found(reason: &str) -> Self` — Decidable but no failure found.
  - (pub fn) `pub fn undecidable(reason: &str) -> Self` — Not decidable by this probe.
  - (pub struct) `pub struct JumpTarget` — A structured jump target for explain/UI integration.
  - (priv impl) `impl JumpTarget`
  - (pub fn) `pub fn from_span(start: usize, end: usize) -> Self`
  - (pub fn) `pub fn with_label(mut self, label: &str) -> Self`
  - (pub fn) `pub fn with_kind(mut self, kind: &str) -> Self`
  - (pub struct) `pub struct ByteSpan` — Byte span for jump targets.
  - (pub struct) `pub struct RefuteLimits` — Resource limits for refutation. Defaults are conservative for fast p95 response times.
  - (priv impl) `impl Default for RefuteLimits`
  - (priv fn) `fn default() -> Self`
  - (pub enum) `pub enum RefuteFragment` — Fragment of theory that a probe can handle. Used for "honest unsupported" - probes decline fragments they can't check.
  - (priv impl) `impl RefuteFragment`
  - (pub fn) `pub fn level(&self) -> u8` — The coherence level this fragment requires.
  - (priv fn) `fn test_bounded_list_from_vec()`
  - (priv fn) `fn test_bounded_list_truncated()`
  - (priv fn) `fn test_refute_limits_default()`

### `src/refute/witness.rs`

Witness types for refutation failures.

Witnesses are pedagogical artifacts, not solver dumps.
They're stored structurally and rendered via PrettyCtx.

- uses:
  - `serde::{Deserialize, Serialize}`
  - `crate::refute::types::{BoundedList, DecisionInfo, JumpTarget}`
  - `crate::refute::probe::ProbeKey`
  - `crate::refute::slice::SliceSummary`
- items:
  - (pub struct) `pub struct Counterexample` — A counterexample found by refutation. This is the payload for `Proposal<CounterexamplePayload>`.
  - (pub struct) `pub struct InterpretationSummary` — Summary of an interpretation for display.
  - (pub enum) `pub enum InterpretationKind` — Tagged interpretation kind for forward compatibility.
  - (pub enum) `pub enum FailureWitness` — A witness to a failure in the interpretation. **Mac Lane-native**: This is not a "model" - it's a failed obligation described in the language of proof objects (explainable + teachable).
  - (pub struct) `pub struct DiagramBoundary` — A diagram boundary requiring a higher cell. Machine-facing structure for UI, explain integration, and diagram workflows.
  - (pub struct) `pub struct CounterexamplePayload` — Payload for `Proposal<CounterexamplePayload>`. Wraps Counterexample with additional context for the proposal protocol.
  - (priv impl) `impl CounterexamplePayload`
  - (pub fn) `pub fn new(counterexample: Counterexample) -> Self`

### `src/span_conversion.rs`
- uses:
  - `source_span::Span`
  - `tower_lsp::lsp_types::{Position, Range}`
  - `super::*`
  - `super::*`
  - `proptest::prelude::*`
- items:
  - (pub fn) `pub fn offset_to_position(text: &str, offset: usize) -> Option<Position>` — Converts a byte offset to an LSP Position (line, character), using UTF-16 code units. This function is the canonical way to convert from internal byte offsets to LSP positions. It handles UTF-16 surrogate pairs correctly, which is required by the LSP spec. Returns None if the offset is out of bounds or falls in the middle of a multi-byte character.
  - (pub fn) `pub fn position_to_offset(text: &str, position: Position) -> Option<usize>` — Converts an LSP Position (line, character) to a byte offset. This is the inverse of `offset_to_position`. It interprets the character index as a count of UTF-16 code units.
  - (pub fn) `pub fn byte_span_to_lsp_range(text: &str, span: Span) -> Option<Range>` — Converts an internal byte Span to an LSP Range. Uses `offset_to_position` for start and end, ensuring consistent UTF-16 handling.
  - (pub fn) `pub fn span_to_lsp_range(span: &Span, source: &str) -> Result<Range, SpanConversionError>` — Converts a Span to an LSP Range with error handling. This is the canonical API for converting SniperDB spans to LSP ranges. Returns Result for better error reporting. # Requirements - Validates: Requirements 5.2, 6.1, 6.2, 6.3 # Errors Returns an error if the span is invalid (out of bounds, negative positions, etc.)
  - (pub fn) `pub fn byte_offset_to_utf16_position(source: &str, byte_offset: usize) -> Option<Position>` — Converts a byte offset to a UTF-16 position. This is an alias for `offset_to_position` with the naming convention from the design document. # Requirements - Validates: Requirements 5.2, 6.1, 6.2
  - (pub enum) `pub enum SpanConversionError` — Error type for span conversion failures.
  - (priv impl) `impl std::fmt::Display for SpanConversionError`
  - (priv fn) `fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result`
  - (priv impl) `impl std::error::Error for SpanConversionError {}`
  - (priv fn) `fn ascii_only()`
  - (priv fn) `fn multibyte_unicode_emoji()`
  - (priv fn) `fn multibyte_cjk()`
  - (priv fn) `fn crlf_line_endings()`
  - (priv fn) `fn empty_spans()`
  - (priv fn) `fn eof_positions()`
  - (priv fn) `fn out_of_bounds()`
  - (priv fn) `fn span_to_lsp_range_valid()`
  - (priv fn) `fn span_to_lsp_range_out_of_bounds()`
  - (priv fn) `fn span_to_lsp_range_invalid_span()`
  - (priv fn) `fn byte_offset_to_utf16_position_alias()`
  - (priv fn) `fn error_display()`
  - (priv fn) `fn regression_emoji_combining_marks_multiline()`
  - (priv fn) `fn mid_character_offsets_return_none()`
  - (priv fn) `fn text_strategy() -> impl Strategy<Value = String>`
  - (priv fn) `fn valid_char_boundary_offsets(text: &str) -> Vec<usize>`
  - (priv fn) `fn prop_offset_position_roundtrip(text in text_strategy())` — Property 14: UTF-16 Conversion Correctness For any valid text and char-boundary offset, converting to position and back should preserve the offset
  - (priv fn) `fn prop_invalid_mid_char_offsets_return_none(text in text_strategy())` — Property 16: Invalid Mid-Character Offsets Return None For any offset that falls in the middle of a multi-byte character, offset_to_position should return None
  - (priv fn) `fn prop_multibyte_utf16_correctness(` — Property 14: UTF-16 Conversion with Multi-byte Characters For any text containing multi-byte characters, UTF-16 positions should be correct
  - (priv fn) `fn prop_invalid_span_handling(text in text_strategy())` — Property 16: Invalid Span Handling For any invalid span, span_to_lsp_range should return an error
  - (priv fn) `fn prop_span_conversion_preserves_boundaries(text in text_strategy())` — Property 14: Span Conversion Preserves Boundaries For any valid span at char boundaries, converting to LSP range should preserve start/end relationships
  - (priv fn) `fn prop_empty_spans(text in text_strategy())` — Property 14: Empty Spans Convert Correctly For any valid char-boundary offset, an empty span should convert to a zero-width range
  - (priv fn) `fn prop_line_boundaries(lines in prop::collection::vec("[a-zA-Z0-9 ]*", 1..10))` — Property 14: Line Boundaries Handled Correctly For any text with newlines, positions should correctly track line numbers

### `src/tactics/edit.rs`
- uses:
  - `tower_lsp::lsp_types::{WorkspaceEdit, TextEdit, Url, Range}`
  - `std::collections::HashMap`
  - `crate::document::ByteSpan`
- items:
  - (pub struct) `pub struct EditBuilder` — Helper to build WorkspaceEdits for tactics.
  - (priv impl) `impl EditBuilder`
  - (pub fn) `pub fn new(uri: Url, text: String) -> Self`
  - (pub fn) `pub fn replace_span(&self, span: ByteSpan, new_text: String) -> WorkspaceEdit` — Create a WorkspaceEdit that replaces the given span with new text.
  - (pub fn) `pub fn insert_before_span(&self, span: ByteSpan, new_text: String) -> WorkspaceEdit` — Create a WorkspaceEdit that inserts text before the given span.
  - (pub fn) `pub fn wrap_span(&self, span: ByteSpan, prefix: String, suffix: String) -> WorkspaceEdit` — Wrap a span with a prefix and suffix.
  - (priv fn) `fn span_to_range(&self, span: ByteSpan) -> Range`
  - (priv fn) `fn offset_to_position(&self, offset: usize) -> tower_lsp::lsp_types::Position`

### `src/tactics/mod.rs`

This module contains the core data models and a minimal runner for the
Mac Lane tactics layer. It serves as a starter slice implementation
based on the `TACTICS_LAYER_SPEC.md`, reusing existing types from the codebase.

- uses:
  - `crate::document::ByteSpan`
  - `crate::refute::types::{BoundedList, StableAnchor}`
  - `codeswitch::fingerprint::HashValue`
  - `serde::{Deserialize, Serialize}`
  - `std::collections::BTreeMap`
- items:
  - (pub struct) `pub struct SemanticPatch`
  - (priv impl) `impl SemanticPatch`
  - (pub fn) `pub fn compute_id(&self) -> HashValue` — Computes a deterministic hash for the patch by canonical serialization. Uses `codeswitch::fingerprint::HashValue::hash_with_domain`.
  - (pub enum) `pub enum PatchKind`
  - (pub enum) `pub enum SemanticChange`
  - (pub enum) `pub enum RewriteDirection`
  - (pub enum) `pub enum InsertPosition`
  - (pub struct) `pub struct Goal`
  - (pub enum) `pub enum ContextItem`
  - (pub struct) `pub struct UIMetadata`
  - (pub struct) `pub struct TacticInput`
  - (pub struct) `pub struct ProofState`
  - (pub struct) `pub struct Hole`
  - (pub enum) `pub enum TacticResult`
  - (pub enum) `pub enum TacticFailureReason`
  - (pub fn) `pub fn run_tactic<F>(input: TacticInput, tactic_fn: F) -> TacticResult` — A minimal tactic runner that executes a tactic function. In a real system, this would involve more complex state management and interaction with the elaborator and Stonewall.
  - (pub fn) `pub fn intro_binder_tactic(input: TacticInput) -> TacticResult` — Implements `intro.binder` logic. Creates a SemanticPatch to introduce a binder.
  - (pub fn) `pub fn exact_term_tactic(input: TacticInput) -> TacticResult` — Implements `exact.term` logic. Creates a SemanticPatch to fill a hole with a given term.
  - (pub fn) `pub fn rewrite_rule_tactic(input: TacticInput) -> TacticResult` — Implements `rewrite.rule` logic. Creates a SemanticPatch to apply a rewrite rule.
  - (pub fn) `pub fn simp_fuel_tactic(input: TacticInput) -> TacticResult` — Implements `simp.fuel` logic (simplified). Creates a SemanticPatch representing a simplification, or fails if fuel is exhausted.

### `src/tactics/query.rs`
- uses:
  - `crate::tactics::view::Selection`
  - `crate::document::ParsedDocument`
  - `crate::document::ByteSpan`
  - `comrade_lisp::proof_state::{ProofState, MorMetaId, GoalStatus}`
  - `std::collections::BTreeSet`
- items:
  - (pub trait) `pub trait TacticQuery` — High-level query interface for tactics.
  - (priv fn) `fn node_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<ByteSpan>;` — Find the innermost AST node at the selection.
  - (priv fn) `fn goal_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<MorMetaId>;` — Find the goal ID at the selection.
  - (priv fn) `fn blockers_for_goal(&self, proof: &ProofState, goal_id: MorMetaId) -> BTreeSet<MorMetaId>;` — Get the set of metas that directly block the given goal.
  - (priv fn) `fn macro_call_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<String>;` — Find if the selection is inside a macro call and return the macro name.
  - (pub struct) `pub struct SemanticQuery;`
  - (priv impl) `impl SemanticQuery`
  - (pub fn) `pub fn new() -> Self`
  - (priv impl) `impl TacticQuery for SemanticQuery`
  - (priv fn) `fn node_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<ByteSpan>`
  - (priv fn) `fn goal_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<MorMetaId>`
  - (priv fn) `fn blockers_for_goal(&self, proof: &ProofState, goal_id: MorMetaId) -> BTreeSet<MorMetaId>`
  - (priv fn) `fn macro_call_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<String>`
  - (priv impl) `impl Default for SemanticQuery`
  - (priv fn) `fn default() -> Self`

### `src/tactics/registry.rs`
- uses:
  - `std::collections::BTreeMap`
  - `std::sync::Arc`
  - `crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction}`
- items:
  - (pub struct) `pub struct TacticRegistry` — A registry for all available tactics. Uses BTreeMap to ensure deterministic iteration order.
  - (priv impl) `impl TacticRegistry`
  - (pub fn) `pub fn new() -> Self`
  - (pub fn) `pub fn register(&mut self, tactic: Arc<dyn Tactic>)` — Register a new tactic.
  - (pub fn) `pub fn compute_all(&self, req: &TacticRequest) -> Vec<TacticAction>` — Compute all applicable actions from all registered tactics.
  - (priv impl) `impl Default for TacticRegistry`
  - (priv fn) `fn default() -> Self`

### `src/tactics/stdlib/goaldirected.rs`
- uses:
  - `crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety}`
  - `crate::tactics::edit::EditBuilder`
  - `comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind}`
  - `std::collections::BTreeMap`
- items:
  - (pub struct) `pub struct FocusGoalTactic;`
  - (priv impl) `impl Tactic for FocusGoalTactic`
  - (priv fn) `fn id(&self) -> &'static str`
  - (priv fn) `fn title(&self) -> &'static str`
  - (priv fn) `fn compute(&self, req: &TacticRequest) -> TacticResult`

### `src/tactics/stdlib/mod.rs`
- modules:
  - `pub mod quickfix;`
  - `pub mod goaldirected;`
  - `pub mod rewrite;`
- uses:
  - `std::sync::Arc`
  - `crate::tactics::registry::TacticRegistry`
- items:
  - (pub fn) `pub fn register_std_tactics(registry: &mut TacticRegistry)`

### `src/tactics/stdlib/quickfix.rs`
- uses:
  - `crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety}`
  - `crate::tactics::edit::EditBuilder`
  - `comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind}`
  - `std::collections::BTreeMap`
- items:
  - (pub struct) `pub struct AddTouchTactic;`
  - (priv impl) `impl Tactic for AddTouchTactic`
  - (priv fn) `fn id(&self) -> &'static str`
  - (priv fn) `fn title(&self) -> &'static str`
  - (priv fn) `fn compute(&self, req: &TacticRequest) -> TacticResult`

### `src/tactics/stdlib/rewrite.rs`

### `src/tactics/view.rs`
- uses:
  - `serde::{Deserialize, Serialize}`
  - `std::collections::BTreeMap`
  - `tower_lsp::lsp_types::{Range, WorkspaceEdit}`
  - `comrade_lisp::proof_state::ProofState`
  - `crate::document::ParsedDocument`
  - `crate::edgelord_pretty_ctx::EdgeLordPrettyCtx`
  - `comrade_lisp::diagnostics::anchors::StableAnchor`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
- items:
  - (pub struct) `pub struct Selection` — Selection details from the editor.
  - (pub struct) `pub struct TacticLimits` — Resource limits for tactic computation.
  - (priv impl) `impl Default for TacticLimits`
  - (priv fn) `fn default() -> Self`
  - (pub struct) `pub struct TacticRequest<'a>` — Input bundle for a tactic.
  - (pub enum) `pub enum TacticResult` — The result of a tactic proposal.
  - (pub enum) `pub enum ActionSafety` — Safety level of a tactic action.
  - (pub enum) `pub enum ActionKind` — Kind of tactic action for UI grouping.
  - (pub struct) `pub struct TacticAction` — A specific proposal from a tactic.
  - (pub trait) `pub trait Tactic: Send + Sync` — Interface for all tactics.
  - (priv fn) `fn id(&self) -> &'static str;` — Stable identifier for registry (e.g., "std.add_touch").
  - (priv fn) `fn title(&self) -> &'static str;` — Human-readable title of the tactic family.
  - (priv fn) `fn compute(&self, req: &TacticRequest) -> TacticResult;` — Compute proposals for a given request.

### `tests/bench_phase1_cache.rs`
- uses:
  - `std::fs`
  - `std::path::Path`
- items:
  - (priv fn) `fn bench_c2_hot_edit()`
  - (priv fn) `fn bench_c2_cross_file()`

### `tests/bench_phase1_report.rs`
- uses:
  - `std::fs`
- items:
  - (priv struct) `struct CsvRow`
  - (priv impl) `impl CsvRow`
  - (priv fn) `fn from_csv_line(line: &str) -> Option<Self>`
  - (priv struct) `struct Stats`
  - (priv impl) `impl Stats`
  - (priv fn) `fn phase1_hit_rate(&self) -> f64`
  - (priv fn) `fn phase1_1_hit_rate(&self) -> f64`
  - (priv fn) `fn combined_hit_rate(&self) -> f64`
  - (priv fn) `fn p50_latency(&self) -> u64`
  - (priv fn) `fn p95_latency(&self) -> u64`
  - (priv fn) `fn parse_csv(path: &str) -> Vec<CsvRow>`
  - (priv fn) `fn compute_stats(rows: &[CsvRow]) -> Stats`
  - (priv fn) `fn bench_c2_generate_report()`
  - (pub struct) `pub struct Local;`
  - (priv impl) `impl Local`
  - (pub fn) `pub fn now() -> String`

### `tests/cache_phase1_1_bench.rs`
- uses:
  - `codeswitch::fingerprint::HashValue`
  - `edgelord_lsp::caching::{CacheKey, CacheValue, ModuleCache}`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `std::time::{Instant, SystemTime}`
  - `std::time::Instant`
- items:
  - (priv struct) `struct BenchmarkRow` — CSV row for benchmark results
  - (priv impl) `impl BenchmarkRow`
  - (priv fn) `fn to_csv_header() -> &'static str`
  - (priv fn) `fn to_csv_line(&self) -> String`
  - (priv fn) `fn make_cache_key(unit_id: &str, content_version: usize, workspace_version: usize) -> CacheKey` — Helper to create a cache key for a specific file version
  - (priv fn) `fn make_cache_value(dv: usize) -> CacheValue` — Helper to create a dummy cache value
  - (priv fn) `fn bench_hot_edit_loop()` — Scenario 1: Hot edit loop on single file Simulate repeatedly editing the same file with high cache hit rate expected.
  - (priv fn) `fn bench_cross_file_edit_loop()` — Scenario 2: Cross-file edit loop Simulate editing multiple dependent files, testing invalidation.
  - (priv fn) `fn bench_cache_under_size_pressure()` — Scenario 3: Cache effectiveness under size pressure Insert many entries and verify eviction behavior.
  - (priv fn) `fn print_benchmark_results(scenario: &str, results: &[BenchmarkRow])` — Print benchmark results in CSV format to stdout
  - (priv fn) `fn test_cache_stats_comprehensive()` — Integration test: Verify stats tracking across operations
  - (priv fn) `fn test_cache_acceptance_thresholds()` — Acceptance threshold verification with baseline measurement
  - (priv fn) `fn test_cache_baseline_comparison()` — Baseline vs cached compilation time comparison

### `tests/cache_phase1_1_invariants.rs`
- uses:
  - `codeswitch::fingerprint::HashValue`
  - `edgelord_lsp::caching::{CacheKey, CacheKeyBuilder, CacheValue, ModuleCache}`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `std::time::SystemTime`
  - `std::collections::BTreeMap`
  - `tower_lsp::lsp_types::Url`
  - `comrade_lisp::WorkspaceDiagnostic`
- items:
  - (priv fn) `fn test_cache_key(` — Helper to create a test cache key
  - (priv fn) `fn test_cache_value(seed: u64) -> CacheValue` — Helper to create a test cache value
  - (priv fn) `fn test_inv_d_cache_1_purity_determinism_replay()` — INV D-CACHE-1: Purity Test Statement: For the same CacheKey, compilation output is identical. Test: Compile twice with identical inputs, verify identical fingerprints.
  - (priv fn) `fn test_inv_d_cache_1_purity_warm_cache_determinism()` — INV D-CACHE-1: Purity Test (Warm Cache) Test: Compile once (populate cache), compile again (hit cache). Verify: Cached output equals fresh compile output.
  - (priv fn) `fn test_inv_d_cache_2_sound_reuse_options_mismatch_busts_cache()` — INV D-CACHE-2: Sound Reuse Test (Options Mismatch) Statement: Cache reuse only if CacheKey matches exactly. Test: Change compile option without changing file. Verify: Cache not used; cache miss recorded.
  - (priv fn) `fn test_inv_d_cache_2_sound_reuse_workspace_mismatch_busts_cache()` — INV D-CACHE-2: Sound Reuse Test (Workspace Mismatch) Test: Change workspace (another file changes). Verify: Cache not used for dependent unit.
  - (priv fn) `fn test_inv_d_cache_2_sound_reuse_content_mismatch_busts_cache()` — INV D-CACHE-2: Sound Reuse Test (Content Mismatch) Test: Change file content. Verify: Cache not used.
  - (priv fn) `fn test_inv_d_cache_3_monotone_invalidation_rollback_re_hit()` — INV D-CACHE-3: Monotone Invalidation Test (Single File Edit) Statement: Edit invalidates cached results for that file. Test: Edit file → miss; revert to previous → hit again. Verify: Second compile is cache hit with identical key.
  - (priv fn) `fn test_inv_d_cache_3_monotone_invalidation_workspace_change()` — INV D-CACHE-3: Monotone Invalidation Test (All-Invalidate on Workspace Change) Test: Change any file in workspace. Verify: All dependent caches invalidated (conservative approach for Phase 1.1).
  - (priv fn) `fn test_cache_statistics_tracking()` — Test: Cache statistics tracking
  - (priv fn) `fn test_cache_key_builder_validation()` — Test: Cache key builder validates all required fields
  - (priv fn) `fn test_cache_key_total_ordering()` — Test: Cache key is totally ordered (BTreeMap determinism)
  - (priv fn) `fn test_workspace_change_invalidates_cache()` — Test: Workspace change invalidates cache (conservative Phase 1.1 behavior)

### `tests/cache_phase1_1_races.rs`
- uses:
  - `codeswitch::fingerprint::HashValue`
  - `edgelord_lsp::caching::{CacheKey, CacheValue, ModuleCache}`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `std::sync::{Arc, Mutex}`
  - `std::time::SystemTime`
  - `std::collections::BTreeMap`
  - `tower_lsp::lsp_types::Url`
  - `comrade_lisp::WorkspaceDiagnostic`
- items:
  - (priv fn) `fn test_cache_key_with_dv(dv: i32, content: &str) -> CacheKey` — Helper to create a test cache key with DV
  - (priv fn) `fn test_cache_value_with_dv(dv: i32) -> CacheValue` — Helper to create a test cache value with DV fingerprint
  - (priv fn) `fn test_inv_d_race_1_single_flight_concurrent_requests()` — INV D-RACE-1: Single-Flight Gate Test Statement: At most one in-flight compile per unit per DV. Test: Simulate concurrent requests for same unit; verify only latest compiles.
  - (priv fn) `fn test_inv_d_race_2_no_stale_diagnostics_out_of_order_completion()` — INV D-RACE-2: No Stale Diagnostics Test Statement: Published diagnostics must correspond to newest DV at publish time. Test: Simulate out-of-order completion; verify old DV never publishes.
  - (priv fn) `fn test_inv_d_race_3_cache_hit_cannot_overwrite_newer_dv()` — INV D-RACE-3: Cache Hit Cannot Overwrite Newer DV Statement: Late cache-hit must not publish over a newer DV. Test: DV1 cache-hit completes slowly; DV2 finishes first; verify DV1 doesn't overwrite DV2.
  - (priv fn) `fn test_cache_deterministic_ordering_under_concurrent_inserts()` — Verification: Cache maintains deterministic ordering (no race in lookup/insert)
  - (priv fn) `fn test_single_flight_gate_pattern()` — Test: Single-flight gate pattern (as would be used in ProofSession)
  - (priv struct) `struct SingleFlightGate`
  - (priv impl) `impl SingleFlightGate`
  - (priv fn) `fn acquire(&mut self, dv: i32) -> bool`
  - (priv fn) `fn is_current(&self, dv: i32) -> bool`

### `tests/db7_hover_preview.rs`
- uses:
  - `std::sync::Arc`
  - `tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufStream, duplex, AsyncReadExt}`
  - `tokio::time::{timeout, Duration, Instant}`
  - `serde_json::{json, Value}`
  - `tokio::sync::RwLock`
  - `edgelord_lsp::lsp::{Backend, Config}`
- items:
  - (priv fn) `async fn send_message(stream: &mut BufStream<tokio::io::DuplexStream>, message: Value)`
  - (priv fn) `async fn read_one_message_timeout(`
  - (priv fn) `async fn read_until_response(`
  - (priv fn) `async fn setup_server() -> (`
  - (priv fn) `async fn initialize_server(`
  - (priv fn) `async fn open_document(`
  - (priv fn) `async fn send_hover_request(`
  - (priv fn) `async fn test_smoke_server_lifecycle()`
  - (priv fn) `async fn test_db7_hover_rename_preview_appears()`
  - (priv fn) `async fn test_db7_hover_whitespace_fail_closed()`
  - (priv fn) `async fn test_db7_hover_punctuation_fail_closed()`
  - (priv fn) `async fn test_db7_hover_cache_stability()`
  - (priv fn) `async fn test_db7_hover_file_sync_sanity()`
  - (priv fn) `async fn test_db7_code_action_rename_preview()`

### `tests/diagnostic_publishing_tests.rs`
- uses:
  - `edgelord_lsp::lsp::PublishDiagnosticsHandler`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `comrade_lisp::{WorkspaceDiagnostic, WorkspaceDiagnosticSeverity}`
  - `source_span::Span`
  - `tower_lsp::lsp_types::{DiagnosticSeverity, Url}`
  - `std::collections::BTreeMap`
  - `super::*`
  - `proptest::prelude::*`
  - `super::*`
  - `edgelord_lsp::document::ParsedDocument`
  - `tower_lsp::lsp_types::Url`
- items:
  - (priv fn) `fn severity_strategy() -> impl Strategy<Value = WorkspaceDiagnosticSeverity>`
  - (priv fn) `fn prop_canonical_pipeline_completeness(` — Property 1: Canonical Pipeline Completeness For any file elaboration with N errors, all N errors SHALL be collected through the canonical pipeline and published to LSP, never through intermediate paths. Validates: Requirements 1.1, 1.2, 1.3
  - (priv fn) `fn prop_diagnostic_sorting_determinism(` — Property 15: Diagnostic Sorting Determinism For any unordered set of diagnostics, sorting should produce consistent results
  - (priv fn) `fn prop_lsp_integration_completeness(` — Property 10: LSP Integration Completeness For any file with N diagnostics, querying the LSP should return all N diagnostics Validates: Requirements 8.3, 8.5
  - (priv fn) `fn test_empty_diagnostics()`
  - (priv fn) `fn test_diagnostic_with_code()`
  - (priv fn) `fn test_diagnostic_source_field()`
  - (priv fn) `fn test_lsp_multi_diagnostic_publication()` — Integration test for LSP multi-diagnostic publication Validates: Requirements 8.3, 8.5
  - (priv fn) `fn test_acceptance_determinism_verification()` — Acceptance test: Determinism verification - same file elaborated multiple times Validates: Requirements 2.1, 10.2, 10.3
  - (priv fn) `fn test_error_clearing_through_canonical_pipeline()` — Integration test for error clearing through canonical pipeline Validates: Requirements 1.4
  - (priv fn) `fn test_prop_error_clearing_completeness()` — Property test for error clearing completeness For any error fixed, verify diagnostic is cleared through canonical pipeline Validates: Requirements 1.4
  - (priv fn) `fn test_inv_t_dvcmp_early_check_rejects_stale_work()` — Integration test: Document-version guard prevents stale ScopeCreep results **INV T-DVCMP Validation**: Verifies that stale Phase 2 (ScopeCreep) results are rejected if the document has changed during async analysis. Scenario: 1. Document at version 1 → Phase 1 publishes, Phase 2 spawned 2. User edits to version 2 → Phase 1 publishes for v2, Phase 2 spawned for v2 3. Phase 2 v1 completes first (was slower) → REJECTED (version mismatch) 4. Phase 2 v2 completes → ACCEPTED (version matches) This test validates the early check: document version is checked before ScopeCreep analysis starts.
  - (priv fn) `fn test_inv_t_dvcmp_late_check_prevents_stale_publish()` — Integration test: Late check prevents publishing stale Phase 2 results **INV T-DVCMP Validation**: Verifies that Phase 2 results from analysis are not published if the document version changed during analysis. Scenario: 1. Phase 2 starts for v1, captures version guard = 1 2. Analysis runs asynchronously... 3. Meanwhile, document advances to v2 (late check sees version 2) 4. Analysis completes → Late check rejects publish (version mismatch) This test validates the late check: document version is verified again after analysis completes, before publishing merged results.
  - (priv fn) `fn test_inv_t_dvcmp_rapid_edits_only_latest_publishes()` — Integration test: Document version tracking across multiple edits **INV T-DVCMP Coverage**: Verifies that document versions can be tracked and compared reliably across rapid edits. Validates: - Version increments on each edit - Early check catches version mismatches - Late check prevents stale publishes - Only the latest version's Phase 2 results publish

### `tests/diff_tests.rs`
- uses:
  - `edgelord_lsp::diff::engine::compute_diff`
  - `edgelord_lsp::goals_panel::GoalChangeKind`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `comrade_lisp::diagnostics::DiagnosticContext`
  - `source_span::Span`
  - `comrade_lisp::proof_state`
- items:
  - (priv fn) `fn test_status_change_diff()`
  - (priv fn) `fn test_blockers_change_diff()`

### `tests/explain_tests.rs`
- uses:
  - `edgelord_lsp::explain::builder::ExplainBuilder`
  - `edgelord_lsp::explain::view::{ExplanationKind, ExplainLimits, validate_span}`
  - `edgelord_lsp::explain::alg_goal::explain_goal`
  - `edgelord_lsp::explain::alg_blocked::explain_why_blocked`
  - `edgelord_lsp::explain::alg_inconsistent::explain_why_inconsistent`
  - `comrade_lisp::proof_state`
  - `comrade_lisp::diagnostics::projection::GoalsPanelIndex`
  - `comrade_lisp::diagnostics::DiagnosticContext`
  - `source_span::Span`
- items:
  - (priv fn) `fn test_builder_determinism()`
  - (priv fn) `fn test_jump_target_validity()`
  - (priv fn) `fn test_explain_why_blocked_snapshot()`
  - (priv fn) `fn test_explain_goal_snapshot()`
  - (priv fn) `fn test_explain_why_inconsistent_snapshot()`

### `tests/highlight_test.rs`
- uses:
  - `edgelord_lsp::highlight::{compute_layer0_structural, tokens_to_lsp_data, SymbolRole}`
- items:
  - (priv fn) `fn test_layer1_highlighting_valid()`
  - (priv fn) `fn test_layer0_fallback_invalid()`

### `tests/integration_tests.rs`
- uses:
  - `std::{sync::Arc}`
  - `tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufStream, duplex, AsyncReadExt}`
  - `tokio::time::{timeout, Duration, Instant}`
  - `serde_json::{json, Value}`
  - `tokio::sync::RwLock`
  - `edgelord_lsp::{lsp::{Backend, Config}}`
- items:
  - (priv fn) `async fn send_message(stream: &mut BufStream<tokio::io::DuplexStream>, message: Value)`
  - (priv fn) `async fn read_one_message(stream: &mut BufStream<tokio::io::DuplexStream>) -> Option<Value>`
  - (priv fn) `async fn test_initialize_did_open_publishes_diagnostics()`
  - (priv fn) `async fn test_debounce_and_single_flight()`
  - (priv fn) `async fn test_workspace_report_integration_with_latency()`
  - (priv fn) `async fn test_workspace_report_rapid_changes()`

### `tests/loogle_tests.rs`
- uses:
  - `edgelord_lsp::loogle::{LoogleIndex, WorkspaceIndexer, check_applicability}`
  - `edgelord_lsp::loogle::LoogleResult`
  - `edgelord_lsp::loogle::{LoogleResult, ApplicabilityResult, to_proposal}`
  - `edgelord_lsp::loogle::{LoogleResult, ApplicabilityResult, to_proposal}`
  - `edgelord_lsp::loogle::{compute_fingerprint, LOOGLE_FP_VERSION}`
  - `tcb_core::ast::MorphismTerm`
  - `tcb_core::id_minting::GeneratorId`
  - `edgelord_lsp::loogle::{compute_fingerprint, LOOGLE_FP_VERSION}`
  - `tcb_core::ast::MorphismTerm`
  - `edgelord_lsp::loogle::compute_fingerprint`
  - `tcb_core::ast::MorphismTerm`
  - `tcb_core::doctrine::DoctrineKey`
  - `tcb_core::id_minting::GeneratorId`
- items:
  - (priv fn) `fn test_loogle_index_and_search()`
  - (priv fn) `fn test_applicability_check()`
  - (priv fn) `fn test_proposal_generation()`
  - (priv fn) `fn test_proposal_id_determinism()`
  - (priv fn) `fn test_fingerprint_determinism()`
  - (priv fn) `fn test_fingerprint_version_tag()`
  - (priv fn) `fn test_fingerprint_no_debug_format()`

### `tests/mvp0_utilities.rs`
- uses:
  - `tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent}`
- items:
  - (priv fn) `fn utf16_position_offset_roundtrip_is_stable()`
  - (priv fn) `fn incremental_changes_apply_deterministically()`
  - (priv fn) `fn incremental_changes_respect_input_order()`
  - (priv fn) `fn top_level_symbols_order_is_stable()`
  - (priv fn) `fn selection_chain_validation_rejects_non_nested_spans()`

### `tests/mvp1_2_binders.rs`
- uses:
  - `edgelord_lsp::document::{BindingKind, ParsedDocument}`
- items:
  - (priv fn) `fn let_binder_is_visible_inside_body()`
  - (priv fn) `fn nested_lets_shadow_outer_binders()`
  - (priv fn) `fn local_let_bindings_precede_top_level_bindings()`
  - (priv fn) `fn let_list_binders_are_left_to_right()`

### `tests/mvp1_goals.rs`
- uses:
  - `edgelord_lsp::document::ParsedDocument`
- items:
  - (priv fn) `fn finds_unsolved_goals_with_stable_ids()`
  - (priv fn) `fn goal_context_includes_prior_top_level_bindings()`
  - (priv fn) `fn goal_lookup_by_offset_works()`

### `tests/mvp1_inlay.rs`
- uses:
  - `edgelord_lsp::document::{ByteSpan, ParsedDocument}`
- items:
  - (priv fn) `fn inlay_hints_are_deterministic()`
  - (priv fn) `fn inlay_hints_respect_requested_range()`
  - (priv fn) `fn inlay_hints_order_is_stable_with_same_offset()`

### `tests/phase1_2b_compile_query.rs`
- uses:
  - `edgelord_lsp::queries::{CompileInputV1, Q_CHECK_UNIT_V1, DiagnosticsArtifactV1}`
  - `std::collections::BTreeMap`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
- items:
  - (priv fn) `fn test_compile_input_v1_purity()`
  - (priv fn) `fn test_compile_input_v1_content_sensitivity()`
  - (priv fn) `fn test_compile_input_v1_options_sensitivity()`
  - (priv fn) `fn test_compile_input_v1_workspace_sensitivity()`
  - (priv fn) `fn test_compile_input_v1_file_id_sensitivity()`
  - (priv fn) `fn test_q_check_unit_v1_identity()`
  - (priv fn) `fn test_diagnostics_artifact_v1_creation()`
  - (priv fn) `fn test_compile_input_snapshot_ordering()`
  - (priv fn) `fn test_compile_input_options_ordering()`
  - (priv fn) `fn test_compile_input_hash_stability()`

### `tests/phase1_module_snapshots.rs`
- uses:
  - `std::sync::Arc`
  - `edgelord_lsp::caching::{ModuleSnapshot, ModuleSnapshotCache}`
  - `comrade_lisp::comrade_workspace::WorkspaceReport`
  - `codeswitch::fingerprint::HashValue`
  - `std::time::SystemTime`
- items:
  - (priv fn) `fn default_opts_fp() -> HashValue`
  - (priv fn) `fn default_deps_fp() -> HashValue`
  - (priv fn) `fn test_phase1_module_snapshot_hit_on_all_inputs_match()`
  - (priv fn) `fn test_phase1_module_snapshot_miss_on_content_change()`
  - (priv fn) `fn test_phase1_multiple_files_independent_snapshots()`
  - (priv fn) `fn test_phase1_snapshot_cache_capacity()`
  - (priv fn) `fn test_phase1_snapshot_stats_tracking()`

### `tests/prelude_actual_compilation.rs`

CRITICAL TEST: Does prelude.maclane actually compile?

This test verifies end-to-end compilation of prelude.maclane through the real pipeline.
No mocks, no hypotheticals - actual compilation results.

- uses:
  - `std::fs`
  - `std::path::Path`
- items:
  - (priv fn) `fn test_prelude_maclane_actually_exists_and_is_readable()`
  - (priv fn) `fn test_prelude_maclane_has_valid_sexpression_structure()`
  - (priv fn) `fn test_prelude_maclane_structure_summary()`
  - (priv fn) `fn test_prelude_maclane_compiles_through_pipeline()`
  - (priv fn) `fn test_prelude_maclane_critical_symbols_present()`

### `tests/prelude_compilation_test.rs`

Test: Does prelude.maclane actually compile?

This test attempts to compile the actual prelude file through the
full MacLane compilation pipeline to verify it has no errors.

- uses:
  - `std::fs`
  - `std::path::Path`
- items:
  - (priv fn) `fn test_prelude_actually_compiles()`
  - (priv fn) `fn test_prelude_major_sections()`

### `tests/prelude_demo_integration.rs`

Prelude Demo: End-to-End Integration Test

**Purpose**: Prove the complete two-phase publish story works with prelude.maclane.

**Story**:
1. Open prelude.maclane → Phase 1 publishes Core diagnostics (< 10ms)
2. Wait for Phase 2 → ScopeCreep diagnostics merged (200-500ms)
3. Edit same content → Cache hit (< 1ms from snapshot cache)
4. Navigation works → Go-to-def, find-references functional

**Success Criteria**:
- Phase 1 latency < 10ms ✅
- Phase 2 publishes after Phase 1 ✅
- Stale Phase 2 rejected on rapid edit ✅
- Cache hit on unchanged content ✅
- Diagnostics properly tagged (Core vs ScopeCreep) ✅

- uses:
  - `std::fs`
  - `std::path::Path`
  - `std::time::Instant`
  - `tower_lsp::lsp_types::Url`
  - `std::collections::hash_map::DefaultHasher`
  - `std::hash::{Hash, Hasher}`
  - `comrade_lisp::diagnostics::DiagnosticOrigin`
- items:
  - (priv struct) `struct PreludeDemoConfig` — Prelude demo configuration
  - (priv impl) `impl Default for PreludeDemoConfig`
  - (priv fn) `fn default() -> Self`
  - (priv fn) `fn test_prelude_file_exists_and_valid()`
  - (priv fn) `fn test_phase1_core_diagnostics_instant()`
  - (priv fn) `fn test_phase2_deferred_async()`
  - (priv fn) `fn test_stale_phase2_rejected_on_rapid_edit()`
  - (priv struct) `struct Phase2Task`
  - (priv fn) `fn test_module_snapshot_cache_hit()`
  - (priv fn) `fn compute_hash(s: &str) -> u64`
  - (priv fn) `fn test_diagnostics_properly_tagged()`
  - (priv fn) `fn test_prelude_navigation_ready()`
  - (priv fn) `fn test_complete_story_summary()`

### `tests/refute_golden_check.rs`
- uses:
  - `edgelord_lsp::refute::lsp_handler::{RefuteResponse, RefuteMeta, REFUTE_PROTOCOL_VERSION}`
  - `edgelord_lsp::refute::types::{BoundedList, RefuteLimits, DecisionInfo, JumpTarget, ByteSpan, StableAnchor, AnchorKind, TruncationReason}`
  - `edgelord_lsp::refute::witness::{Counterexample, CounterexamplePayload, FailureWitness, InterpretationSummary, InterpretationKind, DiagramBoundary}`
  - `edgelord_lsp::refute::probe::ProbeKey`
  - `edgelord_lsp::refute::slice::SliceSummary`
  - `edgelord_lsp::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary}`
- items:
  - (priv fn) `fn test_manual_golden_json_strict()`

### `tests/refute_pipeline_integration.rs`
- uses:
  - `edgelord_lsp::refute::lsp_handler::{handle_refute_request, RefuteRequest, REFUTE_PROTOCOL_VERSION}`
- items:
  - (priv fn) `fn test_refute_pipeline_integration()`

### `tests/refute_tests.rs`

Golden determinism tests for refute endpoint (G5).

These tests lock the JSON schema and ensure byte-for-byte determinism.

- uses:
  - `edgelord_lsp::refute::witness::FailureWitness`
  - `edgelord_lsp::refute::types::RefuteLimits`
- items:
  - (priv fn) `fn test_refute_determinism_json_golden()` — Golden test: same input twice → identical JSON (G5)
  - (priv fn) `fn test_refute_level1_returns_missing_coherence_not_false()` — Golden test: coherence level 1 returns MissingCoherenceWitness, not equation failure (G4)
  - (priv fn) `fn test_lsp_refute_smoke()` — LSP smoke test: handler returns valid JSON, no Debug formatting (G1)
  - (priv fn) `fn test_refute_proposals_deterministic_ordering()` — Test deterministic ordering of proposals
  - (priv fn) `fn test_refute_json_schema_snapshot()` — Snapshot the golden JSON schema structure
  - (priv fn) `fn test_refute_slice_is_bounded_and_sorted()` — Slice bounded and sorted test (G2)

### `tests/selection_and_diagnostics.rs`
- uses:
  - `edgelord_lsp::document::{ByteSpan, ParsedDocument, selection_chain_is_well_formed}`
- items:
  - (priv fn) `fn selection_chain_expands_atom_list_form_root()`
  - (priv fn) `fn parse_error_produces_stable_diagnostic()`

### `tests/smoke_test.rs`
- uses:
  - `std::sync::Arc`
  - `tokio::io::{AsyncWriteExt, BufStream, duplex}`
  - `tokio::time::{timeout, Duration}`
  - `tower_lsp::{LspService, Server, lsp_types::InitializeParams}`
  - `serde_json::{json, Value}`
  - `tokio::sync::RwLock`
  - `edgelord_lsp::lsp::{Backend, Config}`
  - `tokio::io::AsyncBufReadExt`
  - `tokio::io::AsyncReadExt`
- items:
  - (priv fn) `async fn send_message(stream: &mut BufStream<tokio::io::DuplexStream>, message: Value)`
  - (priv fn) `async fn read_one_message(stream: &mut BufStream<tokio::io::DuplexStream>) -> Option<Value>`
  - (priv fn) `async fn smoke_test_server_starts()`
  - (priv fn) `async fn smoke_test_initialize_and_shutdown()`

### `tests/tactics_starter_tests.rs`
- uses:
  - `edgelord_lsp::tactics::*`
  - `crate::refute::types::{BoundedList, StableAnchor, AnchorKind}`
  - `codeswitch::fingerprint::HashValue`
  - `std::collections::BTreeMap`
- items:
  - (priv fn) `fn create_test_proof_state() -> ProofState`
  - (priv fn) `fn create_test_tactic_input(`
  - (priv fn) `fn test_intro_binder_tactic_determinism()`
  - (priv fn) `fn test_exact_term_tactic_determinism()`
  - (priv fn) `fn test_rewrite_rule_tactic_determinism()`
  - (priv fn) `fn test_simp_fuel_tactic_determinism()`
  - (priv fn) `fn simulate_stonewall_check(patch: &SemanticPatch) -> bool` — Placeholder for a function that would simulate the kernel's soundness check.
  - (priv fn) `fn test_tactic_produces_sound_patch()`
  - (priv fn) `fn test_tactic_simulates_unsound_patch_failure()`
  - (priv fn) `fn test_semantic_patch_is_independent_of_text_edits()`
  - (priv fn) `fn test_simp_fuel_exhausted()`

### `tests/tactics_tests.rs`
- uses:
  - `edgelord_lsp::edgelord_pretty_ctx::EdgeLordPrettyCtx`
  - `edgelord_lsp::document::ParsedDocument`
  - `comrade_lisp::proof_state::{ProofState, MetaSubst, ElaborationTrace}`
  - `tower_lsp::lsp_types::{Range, Position, Url}`
  - `std::sync::Arc`
- items:
  - (priv fn) `fn test_registry_compute_all()`
  - (priv fn) `fn test_add_touch_tactic_skips_if_exists()`
  - (priv fn) `fn test_focus_goal_tactic()`

### `tests/two_phase_publish_harness.rs`

Two-Phase Publish Contract Test Harness

**Purpose**: Prove that the LSP diagnostic publishing follows the two-phase pattern:
- Phase 1: Core diagnostics published immediately
- Phase 2: Core+ScopeCreep diagnostics published asynchronously

**Invariants Validated**:
- INV D-PUBLISH-CORE: Phase 1 publishes Core-only
- INV D-NONBLOCKING: Phase 2 is async (doesn't delay Phase 1)
- INV T-DVCMP: Phase 2 never publishes for stale document versions
- INV T-MERGE-ORDER: Phase 2 Core subset equals Phase 1 (byte-for-byte)

This harness records all publish events and validates the contract holds
across multiple files, versions, and ScopeCreep behaviors.

- uses:
  - `std::collections::HashMap`
  - `std::sync::{Arc, Mutex}`
  - `tower_lsp::lsp_types::{Diagnostic, Url}`
  - `std::collections::hash_map::DefaultHasher`
  - `std::hash::{Hash, Hasher}`
  - `super::*`
- items:
  - (pub struct) `pub struct PublishEvent` — Records a single publish event for testing.
  - (priv fn) `fn hash_diagnostics(diags: &[Diagnostic]) -> u64` — Hash a list of diagnostics for equality testing.
  - (pub struct) `pub struct PublishEventSink` — Test sink that records all publish events.
  - (priv impl) `impl PublishEventSink`
  - (pub fn) `pub fn new() -> Self` — Create a new sink.
  - (pub fn) `pub fn record(` — Record a publish event.
  - (pub fn) `pub fn events(&self) -> Vec<PublishEvent>` — Get all recorded events.
  - (pub fn) `pub fn clear(&self)` — Clear recorded events.
  - (pub fn) `pub fn events_for_uri(&self, uri: &Url) -> Vec<PublishEvent>` — Get events for a specific URI.
  - (pub fn) `pub fn last_event(&self) -> Option<PublishEvent>` — Get last event (for simple single-file tests).
  - (pub struct) `pub struct TwoPhasePublishHarness` — Harness for validating two-phase publish contract.
  - (priv impl) `impl TwoPhasePublishHarness`
  - (pub fn) `pub fn new() -> Self` — Create a new harness.
  - (pub fn) `pub fn sink(&self) -> &PublishEventSink` — Get the underlying sink (for recording events).
  - (pub fn) `pub fn assert_phase1_core_only(&self, uri: &Url)` — **INV D-PUBLISH-CORE**: Validate that Phase 1 publishes Core-only diagnostics. Assertion: All sources in Phase 1 event are "maclane-core".
  - (pub fn) `pub fn assert_phase2_merged_canonical(&self, uri: &Url)` — **INV T-MERGE-ORDER**: Validate Phase 2 publishes Core+ScopeCreep merged. Assertions: - Phase 2 event exists - Sources contain both "maclane-core" and/or "maclane-scopecreep" - Core diagnostics appear before ScopeCreep in source list
  - (pub fn) `pub fn assert_phase2_version_guard(&self, uri: &Url)` — **INV T-DVCMP**: Validate that Phase 2 never publishes for stale versions. Assertions: - Each Phase 2 publish has a matching Phase 1 publish with same version - Stale versions (Phase 2 without Phase 1) are not published
  - (pub fn) `pub fn assert_phase2_core_equals_phase1(&self, uri: &Url)` — **Byte-for-byte equality**: Phase 2 Core subset equals Phase 1 payload. Assertions: - Find the most recent Phase 1 event for the URI - Find the corresponding Phase 2 event - Core diagnostics hashes must be identical (modulo ScopeCreep additions) **Note**: This is validated by the fact that Phase 1 publishes, then Phase 2 is spawned with the same Core diagnostics payload. The harness doesn't have direct access to verify byte-for-byte equality, but the version guard and canonical ordering ensure it.
  - (pub fn) `pub fn assert_phase1_before_phase2(&self, uri: &Url)` — **INV D-NONBLOCKING**: Validate that Phase 1 publishes before Phase 2 in time. Assertions: - For each document version, Phase 1 event precedes Phase 2 event in record order
  - (priv fn) `fn test_two_phase_core_only_to_merged()` — Test 1: Basic Phase 1 publishes Core-only, Phase 2 merges. **Validates**: INV D-PUBLISH-CORE, INV T-MERGE-ORDER
  - (priv fn) `fn test_two_phase_stale_version_rejected()` — Test 2: Stale versions are never published (INV T-DVCMP). **Scenario**: 1. Version 1 publishes Phase 1 2. Version 2 publishes Phase 1 3. Only Phase 2 for version 2 should publish (version 1 is stale)
  - (priv fn) `fn test_two_phase_rapid_edits_out_of_order_completion()` — Test 3: Rapid edits with out-of-order Phase 2 completion. **Scenario**: 1. Version 1 → Phase 1, Phase 2 spawned 2. Version 2 → Phase 1, Phase 2 spawned (v1 Phase 2 still running) 3. Version 2 Phase 2 completes first → publish 4. Version 1 Phase 2 completes → reject (stale)
  - (priv fn) `fn test_two_phase_multiple_files()` — Test 4: Multiple files maintain independent phase ordering. **Validates**: Harness works correctly with multiple URIs
  - (priv fn) `fn test_two_phase_no_diagnostics()` — Test 5: No diagnostics case (empty Phase 1). **Validates**: Harness handles edge case of clean files
