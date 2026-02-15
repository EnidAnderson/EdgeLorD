# Phase C2 Agent Spec: ProofSession-Integrated, Decision-Grade Benchmarks

**Objective**

Replace the "mock CSV harness" with real measurements from ProofSession::update() on a real fixture workspace, producing:
- `benchmarks/PHASE_1_BASELINE.csv` (caches disabled)
- `benchmarks/PHASE_1_CACHED.csv` (normal caches enabled)
- `benchmarks/PHASE_1_REPORT.md` (P50/P95 + hit rates + compilations per 100 edits + deltas)

This phase is explicitly meant to enable a go/no-go decision for Phase 1.2B (DB memoization) and Phase 1.2C (fine-grained deps).

---

## Scope and Containment

**Allowed edits**
- EdgeLorD/src/ (instrumentation, benchmark harness, cache-disable switch)
- EdgeLorD/tests/ (benchmark test runner)
- EdgeLorD/tests/fixtures/ (fixture workspace files)
- EdgeLorD/benchmarks/ (CSV + report outputs)

**Disallowed edits**
- clean_kernel/ crates unrelated to EdgeLorD unless required to compile EdgeLorD
- SniperDB internals, except importing existing APIs

**Non-goals (explicit)**
- No Phase 1.2B memoization work in this task
- No fine-grained dependency graph work in this task
- No perf optimizations beyond measurement hooks

---

## Deliverables

### D1. Fixture workspace (real A→B→C + unicode + deterministic diagnostic)

Create directory: `EdgeLorD/tests/fixtures/benchmark_workspace/`

Files:
1. `A.mc` (imports B)
2. `B.mc` (imports C)
3. `C.mc` (contains a deterministic error and unicode)
4. `README.md` (explains semantics and why these exact contents exist)

**Fixture requirements**
- Import chain: A → B → C in whatever syntax your surface language uses
- Unicode stressors must appear in the workspace:
  - one ZWJ sequence (e.g., family emoji 👨‍👩‍👧‍👦)
  - one combining mark sequence (e\u{0301} not precomposed é)
  - at least one surrogate-pair emoji (🦀)
- Deterministic diagnostic:
  - Must reliably produce at least one diagnostic with a stable span
  - Prefer a parse-level error (more stable than typecheck stubs)
  - The error should be local to C so the cross-file scenario exercises invalidation

---

### D2. Cache-disable mode (baseline run)

Implement a single knob that disables both Phase 1 and Phase 1.1 caches.

**Required behavior**
- When disabled, code paths still run as normally as possible except cache lookup/insert is bypassed
- Must disable:
  - ModuleSnapshotCache (Phase 1)
  - ModuleCache (Phase 1.1)
- Must not disable single-flight or DV handling

**Implementation options (choose one)**

Preferred: environment variable
```
EDGELORD_DISABLE_CACHES=1
```

Alternative: constructor/config flag
```rust
ProofSession::new(..., caches_enabled: bool)
// or
Config { caches_enabled: bool }
```

**Acceptance test**
- A unit test verifying that when disabled:
  - cache lookups return None
  - cache_entries_* remains 0 (or unchanged) after edits
  - compiled=1 for edits that would otherwise hit

---

### D3. Measurement schema (19 fields) + deterministic row emission

**CSV columns (exact order)**

```
timestamp_ms, scenario, uri, edit_id, dv,
phase1_outcome, phase1_1_outcome, compiled,
compile_ms, end_to_end_ms,
diagnostics_count, bytes_open_docs,
cache_entries_phase1, cache_entries_phase1_1,
options_fp8, deps_fp8, workspace_fp8,
published, note
```

**Outcome encoding (structured enum → string)**

`phase1_outcome` format:
- `hit`
- `miss:content_changed`
- `miss:options_changed`
- `miss:deps_changed`
- `miss:key_unavailable`
- `miss:cache_disabled`
- `miss:eviction`
- `miss:other`

`phase1_1_outcome` format:
- `hit`
- `miss:content_changed`
- `miss:options_changed`
- `miss:workspace_hash_changed`
- `miss:key_unavailable`
- `miss:cache_disabled`
- `miss:eviction`
- `miss:other`

**Determinism requirements**
- The CSV must be identical across runs except timestamp_ms (and optionally note if you include variable text—avoid variable note content)
- Rows must be emitted in a deterministic order:
  - scenario order fixed
  - edit order fixed
  - uri order fixed
- Prefer writing timestamp_ms as an integer from a monotonic reference for that run (still varies, but stable format)

---

### D4. ProofSession::update() instrumentation (must preserve single-flight)

Add measurement hooks inside the existing single-flight choke point.

**Required measurements**
- `end_to_end_ms`: entry → just before publish decision completes
- `compile_ms`: only time spent in compilation path; if cache hit, set compile_ms=0
- `compiled`: 0 if either Phase 1 or Phase 1.1 hits, 1 if compilation performed

**DV + publish tracking**
- `dv`: DV at entry
- `published`: 1 if diagnostics published; 0 if suppressed due to stale DV
- If stale DV suppressed, set note="stale_dv" (constant string)

**Hit detection (must be control-flow based)**
- if Phase 1 cache returns Some, phase1_outcome=hit
- else set phase1_outcome=miss:<reason>
- similarly for Phase 1.1
- Do not infer hits by comparing global hit counters

---

### D5. Miss-reason mapping (explicit code-path contract)

Implement structured miss reasons with an internal enum:

```rust
enum CacheMissReason {
  ContentChanged,
  OptionsChanged,
  DepsChanged,
  WorkspaceHashChanged,
  KeyUnavailable,
  CacheDisabled,
  Eviction,
  Other,
}
```

**Required mapping logic**

Phase 1 (module snapshot) miss reasons:
- `CacheDisabled`: caches disabled
- `KeyUnavailable`: could not compute file_id or content hash / fingerprint
- `OptionsChanged`: options_fp mismatch
- `DepsChanged`: dependency_fingerprint mismatch
- `ContentChanged`: content_hash mismatch
- `Eviction`: cache capacity eviction detected
- `Other`: fallback catch-all

Phase 1.1 (workspace-aware) miss reasons:
- `CacheDisabled`
- `KeyUnavailable`
- `WorkspaceHashChanged`: workspace snapshot hash mismatch
- `OptionsChanged`
- `ContentChanged`
- `Eviction` / `Other`

---

### D6. Benchmark runner test (baseline vs cached)

Create a new ignored test: `EdgeLorD/tests/bench_phase1_cache_real.rs`

It will:
1. Initialize a ProofSession / LSP harness with the fixture workspace opened (A, B, C)
2. Run two scenarios in deterministic edit sequences:
   - hot_edit
   - cross_file
3. Run each scenario twice:
   - baseline: caches disabled
   - cached: caches enabled
4. Produce CSVs and report

**Scenario definitions**

**Scenario S1: hot_edit**
- Open A, B, C (in that order)
- For N=100 edits:
  - edit B: append a whitespace or comment-like harmless change that preserves meaning
  - ensure DV increments each edit

**Scenario S2: cross_file**
- Open A, B, C
- Do the following pattern for N=30 cycles (90 edits total, deterministic):
  1. Edit C (C content_hash changes) → should invalidate A's snapshot
  2. Touch A (no-op edit: re-send exact same bytes) → DV increments, content_hash unchanged
     - Expected for A: `phase1_outcome = miss:deps_changed` (because workspace fingerprint changed due to C)
     - Expected `compiled = 1` (because we should actually re-run work for A; a cache "hit" here would be unsound)
  3. Edit A (A content_hash changes) → direct compilation
     - Expected for A: `phase1_outcome = miss:content_changed`
     - Expected `compiled = 1`

**Implementation note for "touch A"**: Re-send the exact same text for A (or apply a change that cancels out, but ends identical) so DV increments while content_hash does not.

**Output locations**
- `EdgeLorD/benchmarks/PHASE_1_BASELINE.csv`
- `EdgeLorD/benchmarks/PHASE_1_CACHED.csv`

**Test execution**

Must be `#[ignore]` by default, run with:
```bash
cargo test --test bench_phase1_cache_real -- --ignored --nocapture
```

---

### D7. Report generation (decision-grade)

Generate markdown report with:

**Run metadata**
- git commit (if available)
- date/time
- machine info (optional)

**Per scenario**
- baseline:
  - P50/P95 end_to_end_ms
  - compilations per 100 edits
- cached:
  - same metrics
- deltas:
  - % improvement in P95 end_to_end_ms
  - cache hit rates (phase1, phase1.1, combined)
  - compilation reduction %

**Go / no-go decision hooks (for next phases)**

Report must explicitly compute:
- if compilation reduction < 25% and combined hit rate < 40% → likely need 1.2B + 1.2C
- if combined hit rate > 60% → defer 1.2B/1.2C
- if hit rate 40–60% but compile reduction modest → 1.2C likely justified

(These are heuristics; report must present numbers, not conclusions.)

---

## Invariants and Rejection Criteria

**Invariants**
- `INV D-RACE-1`: Measurement must not allow stale DV to publish diagnostics
- `INV D-CACHE-2`: Cache reuse only on exact key match (already enforced; measurement must not subvert it)
- `INV D-BENCH-1`: Benchmark rows must be derived from control flow, not counters

**Reject the change if any of these occur**
- CSV rows depend on nondeterministic note strings (variable text)
- hit/miss classification uses global counters rather than branch outcomes
- baseline run still shows cache entries increasing (cache-disable broken)
- publishing occurs when DV is stale (published=1 when should be 0)
- fixture does not produce any diagnostics at all (diagnostics_count always 0)

---

## Work Plan (Task Breakdown)

**Task C2.1** — Add fixture workspace
- Create the fixture files + README
- Add a small test that opening the fixture yields diagnostics_count >= 1

**Task C2.2** — Add cache-disable knob
- Env var + plumbing to caches
- Add a unit test to verify bypass

**Task C2.3** — Add structured miss reasons + outcome strings
- Introduce enum + mapping functions
- Ensure determinism of string serialization

**Task C2.4** — Add ProofSession instrumentation hooks
- Record one row per edit
- Compute fp8 fields deterministically
- Track published vs stale DV suppression

**Task C2.5** — Implement benchmark runner
- Two scenarios
- Two modes (baseline, cached)
- Write CSVs

**Task C2.6** — Report generator
- Parse CSVs
- Compute P50/P95, hit rates, compilation reduction
- Write markdown report

---

## Agent Checklist (Commands & Debugging)

### 0) Where to run from

```bash
cd EdgeLorD
```

### 1) Quick sanity: tests first

Run the fast/unit checks before generating any CSV:

```bash
cargo test -q
```

If you have ignored-only bench tests, run them explicitly:

```bash
cargo test --test bench_phase1_cache_real -- --ignored --nocapture
```

### 2) Generate the two CSVs (baseline vs cached)

**Baseline (caches disabled)**
```bash
EDGELORD_DISABLE_CACHES=1 cargo test --test bench_phase1_cache_real -- --ignored --nocapture
```

**Cached (normal)**
```bash
unset EDGELORD_DISABLE_CACHES
cargo test --test bench_phase1_cache_real -- --ignored --nocapture
```

### 3) Expected outputs before running the report

You should see two CSVs written (exact names depend on your harness; the spec should enforce them). Expected locations:
- `benchmarks/PHASE_1_BASELINE.csv`
- `benchmarks/PHASE_1_CACHED.csv`

Also expect log lines like:
- "wrote CSV …/PHASE_1_BASELINE.csv"
- "wrote CSV …/PHASE_1_CACHED.csv"
- "rows=N scenario=hot_edit / cross_file"

### 4) Determinism check (required)

Run each mode twice and diff.

**Baseline run 1 & 2**
```bash
EDGELORD_DISABLE_CACHES=1 cargo test --test bench_phase1_cache_real -- --ignored --nocapture
cp benchmarks/PHASE_1_BASELINE.csv /tmp/PHASE_1_BASELINE_1.csv

EDGELORD_DISABLE_CACHES=1 cargo test --test bench_phase1_cache_real -- --ignored --nocapture
cp benchmarks/PHASE_1_BASELINE.csv /tmp/PHASE_1_BASELINE_2.csv

diff -u /tmp/PHASE_1_BASELINE_1.csv /tmp/PHASE_1_BASELINE_2.csv
```

**Cached run 1 & 2**
```bash
unset EDGELORD_DISABLE_CACHES
cargo test --test bench_phase1_cache_real -- --ignored --nocapture
cp benchmarks/PHASE_1_CACHED.csv /tmp/PHASE_1_CACHED_1.csv

cargo test --test bench_phase1_cache_real -- --ignored --nocapture
cp benchmarks/PHASE_1_CACHED.csv /tmp/PHASE_1_CACHED_2.csv

diff -u /tmp/PHASE_1_CACHED_1.csv /tmp/PHASE_1_CACHED_2.csv
```

**Expected**: diffs are empty except fields you intentionally allow to vary.
If you want strict diff-clean determinism, the CSV should not include real wall-clock timestamps; prefer monotone counters or scenario-local "t=0..N" ticks.

### 5) If something fails: inspect these files first

**Measurement + CSV writing**
- `EdgeLorD/tests/bench_phase1_cache_real.rs` (scenario driver, CSV writer, fixture edits)
- `EdgeLorD/benchmarks/PHASE_1_BASELINE.md` (schema + scenario definitions)

**Instrumentation / outcomes**
- `EdgeLorD/src/proof_session.rs` (where outcomes are classified: hit/miss reasons, compiled flag, published flag)
- `EdgeLorD/src/caching.rs` (phase1 + phase1.1 cache get/insert, eviction, disable mode)

**Published vs stale DV suppression**
- `EdgeLorD/src/lsp.rs` (publish funnel + DV gating)

**Span/diagnostic correctness (if ranges look off)**
- `EdgeLorD/src/span_conversion.rs`
- `EdgeLorD/src/diagnostics/*.rs` (or wherever the Diagnostic model + conversion lives)

### 6) "Verified disable" check (must be explicit)

When `EDGELORD_DISABLE_CACHES=1`, confirm in the CSV that:
- `phase1_outcome = miss:cache_disabled`
- `phase1_1_outcome = miss:cache_disabled`
- `compiled = 1` for all rows

If any hit occurs under disable mode, reject (the disable switch is not authoritative).

---

## Notes for the agent

- Keep the instrumentation minimal and local
- Do not widen locks or add extra awaits inside the single-flight section
- Keep note constant strings only ("", "stale_dv", "cache_disabled", etc.)
- Use BTreeMap ordering everywhere if collecting per-uri stats
