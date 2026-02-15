# Phase 1.1 Acceptance Spec: Deterministic Snapshot Reuse (SniperDB-backed)

**Status:** Implementation-grade contract
**Audience:** Implementation agent (Claude)
**Date:** 2026-02-08

---

## Goal

Reduce keystroke→diagnostics latency and redundant recompilation in EdgeLorD by reusing compilation outputs when inputs are unchanged, without sacrificing correctness or determinism.

This phase introduces a `ModuleCache` with a sound cache key, monotone invalidation, and evidence-based measurement. Caching must preserve existing LSP guarantees: single-flight compilation and no stale diagnostics.

---

## 0. Definitions

### 0.1 Terms

- **Document Version (DV):** monotone version counter per file edit (LSP `textDocument/version`)
- **Workspace Snapshot:** stable fingerprint representing all inputs affecting compilation (open docs + on-disk files + dependency graph + compile options)
- **Compile Options:** any settings affecting parsing, elaboration, or checking (profiles, doctrines, feature flags, facet selection)
- **Compilation Output:** complete result EdgeLorD uses for diagnostics and UI:
  - Diagnostics list with stable anchors/locations
  - Canonical fingerprints for caching
  - Report structures (e.g., WorkspaceReport)

### 0.2 "Same inputs" (Exact Match)

"Same inputs" means bitwise identical under this tuple:

```rust
struct CacheKey {
    options_fingerprint: HashValue,      // hash of compile options
    workspace_snapshot_hash: HashValue,   // hash of workspace state
    unit_id: UnitId,                      // file_id or module_id
    unit_content_hash: HashValue,         // hash of file content at DV
    dependency_fingerprint: HashValue,    // hash of transitive deps
}
```

If **any** component differs, reuse is **forbidden**.

---

## 1. Cache Key Construction

### 1.1 Required Key Structure

**CacheKey must include all of:**

1. **OptionsFingerprint**
   - Stable hash of canonical serialization of `CompileOptions`
   - Includes: profiles, doctrines, feature flags, facet selections
   - Computed deterministically (sorted fields for collections)
   - Type: `HashValue` (e.g., SHA-256 or SniperDB's `HashValue::hash_with_domain`)
   - **MUST use canonical serialization:** All map/set collections MUST be BTreeMap/BTreeSet or explicitly sorted before serialization. Debug strings forbidden. Fingerprint MUST be computed from deterministic bytes, then hashed.
   - **INV D-CACHE-1 precondition:** "same options" means bit-for-bit identical canonical bytes.

2. **WorkspaceSnapshotHash**
   - Stable hash representing entire workspace compilation context
   - Includes: set of open documents + their content hashes
   - Can be computed as: `hash_of_sorted_list([(doc_id, doc_hash), ...])`
   - Or derived from SniperDB's workspace snapshot API if available

3. **UnitId**
   - Stable per-compilation-unit identifier
   - Per-file: file path or file_id
   - Per-module: module identifier
   - Must be deterministic (no random IDs)
   - **MUST be canonicalized:** Prefer FileId from ComradeWorkspace when available. Otherwise use a normalized Url string:
     - No fragment, no query
     - Normalized percent-encoding
     - Forward slashes (normalized separators)
     - **Preserve path case** (case-sensitive on Linux; case-insensitive on macOS requires explicit consistent handling)
     - Optionally resolve symlinks only if workspace already canonicalizes paths consistently

4. **UnitContentHash**
   - Hash of the unit's effective content bytes as compiled
   - Post-virtual-doc overlay if applicable
   - Deterministic: same content → same hash, always

5. **DependencyFingerprint**
   - Hash representing transitive dependencies
   - Can be: sorted list of `(dep_id, dep_hash)` hashed
   - Or: derived from SniperDB's dependency graph API
   - Conservative approach: invalidate all on any workspace change (acceptable for Phase 1.1)

### 1.2 Fallback Strategy (Mandatory)

If SniperDB APIs for workspace snapshot and dependency graph are incomplete:

**Fallback for WorkspaceSnapshotHash:**
```
WorkspaceSnapshotHash_fallback = hash_of(
  sorted_list([(doc_path, doc_content_hash) for each open doc])
  + prelude_root_hash_or_workspace_marker
)
```

**Fallback for DependencyFingerprint:**
```
DependencyFingerprint_fallback = WorkspaceSnapshotHash_fallback
// i.e., invalidate all on any workspace change (conservative, safe)
```

**Correctness principle:** Fallback may over-invalidate but must never under-invalidate.

### 1.3 Rejection: No Partial Keys

**Phase 1.1 must NOT ship with:**
- Cache key = `(file_id, content_hash)` only
- Missing OptionsFingerprint
- Missing WorkspaceSnapshotHash or fallback
- Computed at runtime without deterministic hashing

This is an **automatic rejection criterion**.

---

## 2. Cache Invariants (Hard Contracts)

### INV D-CACHE-1: Purity (Determinism)

**Statement:** For the same `CacheKey`, compilation output is identical.

**Operational Meaning:**
- Compile twice with identical inputs → identical outputs
  - Diagnostics: same count, codes, messages, anchors
  - Fingerprints: identical hashes in report
  - Published diagnostics: identical after canonical sorting

**Test Cases:**

1. **Determinism Replay Test**
   - Setup: Open fixture workspace, compile Unit X twice with same DV and options
   - Assert: Canonical serialization of output is identical
   - Failure: Different diagnostics or fingerprints → core breach

2. **Warm-Cache Determinism Test**
   - Setup: Compile once (populate cache), compile again (hit cache)
   - Assert: Cached output byte-for-byte equals fresh compile
   - Failure: Cached output differs → immediate rejection

**Failure Classification:** Core determinism breach (treat as INV D-* violation). Do not paper over in EdgeLorD.

---

### INV D-CACHE-2: Sound Reuse

**Statement:** Cache reuse is allowed only if `CacheKey` matches exactly.

**Test Cases:**

1. **Options Mismatch Busts Cache**
   - Setup: Change compile option (profile/doctrine) without changing file
   - Assert: Cache not used; compilation re-run
   - Instrumentation: Log `cache_miss: options_fingerprint_changed`

2. **Workspace Change Busts Cache**
   - Setup: Change a second file (dependency or in fallback mode)
   - Assert: Dependent unit cache not used
   - Instrumentation: Log `cache_miss: workspace_snapshot_hash_changed`

3. **Dependency Change Busts Cache**
   - Setup: Change imported module; target file unchanged
   - Assert: Dependent compilation not reused
   - Instrumentation: Log `cache_miss: dependency_fingerprint_changed`

**Instrumentation Requirement:** Each compilation decision must emit:
```
{
  timestamp: ...,
  unit_id: ...,
  dv: ...,
  decision: "cache_hit" | "cache_miss",
  miss_reason: "options" | "workspace" | "dependency" | "content" | "new",
  compile_ms: ...
}
```

---

### INV D-CACHE-3: Monotone Invalidation

**Statement:** If an edit changes any relevant input, all affected cached results must be invalidated (or not hit).

**Test Cases:**

1. **Single File Edit Invalidates That File**
   - Setup: Edit within a file
   - Assert: Unit is recompiled (cache miss)
   - Evidence: CSV log shows `cache_miss: content_hash_changed`

2. **Dependent Invalidation**
   - Setup: Edit file B; file A imports B
   - Assert: File A is recompiled (if dependency graph available)
   - Fallback: All files recompiled if dependency unavailable
   - Evidence: CSV log shows invalidation propagation

3. **Rollback Re-Hit**
   - Setup: Edit file → miss → revert to previous content → re-hit
   - Assert: Second compile is cache hit with identical key
   - Confirms: Hash stability and determinism

**Failure:** If any edit fails to invalidate affected caches → immediate rejection.

---

## 3. Race-Condition Guard (Single-Flight Preservation)

### 3.1 Existing LSP Guarantee

EdgeLorD maintains:
- **Single-flight compilation:** at most one in-flight compile per unit per DV
- **No stale diagnostics:** published diagnostics correspond to newest DV observed when publish occurs

### 3.2 Required Implementation Constraint

Cache lookup and diagnostics publish must occur **inside the same single-flight gate:**

```
1. Acquire single-flight token for (UnitId, DV)
2. Compute CacheKey for that DV
3. If cache_hit:
     - Produce output from cache
     - Publish only if DV still current
   Else:
     - Compile
     - Insert into cache
     - Publish only if DV still current
4. Release token
```

**Why:** Prevents late cache-hit from publishing stale output after newer DV starts compiling.

### 3.3 Test Cases

1. **Out-of-Order Completion Simulation**
   - Setup: Force "older DV compile" to complete after newer DV compile
   - Assert: Older output does not publish
   - Mechanism: Deterministic test scheduler or explicit barriers

2. **Cache-Hit Cannot Overwrite Newer DV**
   - Setup: DV1 cache-hit quickly; DV2 slower compile
   - Assert: DV2 publishes, DV1 does not overwrite
   - Evidence: Diagnostics version matches latest DV

**Failure:** If stale diagnostics publish → core LSP contract broken → immediate rejection.

---

## 4. Measurement Methodology (Evidence, not Guesses)

### 4.1 Required Metrics

For a fixed scenario script, record:

| Metric | Definition | Unit |
|--------|-----------|------|
| **Latency P50** | 50th percentile `didChange → diagnostics published` | ms |
| **Latency P95** | 95th percentile `didChange → diagnostics published` | ms |
| **Compilations per 100 keystrokes** | Total compiles / (strokes / 100) | count |
| **Cache hit rate** | hits / (hits + misses) | % |
| **Peak memory** | Peak RSS or approx footprint during session | MB |
| **CPU time** | Total CPU elapsed during scenario | sec |

### 4.2 Scenario Scripts (Must Implement ≥2)

**Scenario 1: Hot Edit Loop (Single File)**
```
Fixture: Small Mac Lane file (100–500 lines)
Steps:
  1. didOpen
  2. Repeat 1000 times:
     - Modify content slightly (e.g., change variable name, add space)
     - Wait for didChange → publish cycle
     - Record metrics
  3. didClose
```

**Scenario 2: Cross-File Edit Loop**
```
Fixture: Two-file workspace, A imports B
Steps:
  1. didOpen A, didOpen B
  2. Repeat 500 times:
     - Edit A (change expression)
     - Edit B (change export)
     - Alternate; record metrics
  3. didClose A, didClose B
```

### 4.3 Output Artifacts (Must Produce)

1. **CSV File: `edgelord_cache_metrics_<timestamp>.csv`**
   ```csv
   timestamp,scenario,dv,unit_id,decision,miss_reason,compile_ms,publish_ms,total_ms,diagnostics_count,memory_mb
   2026-02-08T10:00:00Z,hot_edit_loop,1,file_a.maclane,cache_miss,new,42,5,47,3,15.2
   2026-02-08T10:00:05Z,hot_edit_loop,2,file_a.maclane,cache_hit,,8,5,13,3,15.2
   ...
   ```

2. **Markdown Report: `PHASE_1_1_REPORT.md`**
   - Scenario description
   - Environment info (machine, Rust version, feature flags)
   - Raw numbers (P50/P95 latency, hit rate, memory)
   - Before/after comparison (with --features no_cache)
   - Any anomalies or notes

### 4.4 Acceptance Thresholds

**Go/No-Go criteria for Phase 1.1:**

| Metric | Acceptance Threshold | Notes |
|--------|---------------------|-------|
| Cache hit rate (hot loop) | ≥ 60% | If options/workspace stable |
| Compilations per 100 keystrokes | ≥ 25% reduction vs baseline | vs no_cache run |
| P95 latency improvement | ≥ 20% reduction | vs baseline |
| Correctness | 100% pass rate | All invariants/race tests |

**Fallback:** If thresholds not met but correctness preserved, Phase 1.1 ships as "correctness-first" with a detailed report explaining why hit rate/latency didn't materialize (e.g., "workspace snapshot API incomplete" → forced all-invalidate fallback).

---

## 5. Acceptance Test Harness

### 5.1 Requirements

Provide a deterministic test suite that can:
- Stand up in-memory workspace fixture
- Simulate `didOpen`, `didChange`, `didClose`
- Force DV ordering and completion ordering (for race tests)
- Run scenario scripts and produce CSV + MD outputs
- Support baseline vs cached runs (feature flag or runtime flag)

### 5.2 Required Test Files

```
tests/
├── cache_phase1_1_invariants.rs    // D-CACHE-1, D-CACHE-2, D-CACHE-3
├── cache_phase1_1_races.rs         // Single-flight + no stale output
└── cache_phase1_1_bench.rs         // Scenarios + CSV + report
```

### 5.3 Baseline vs Cached

Tests must support:
```bash
# Baseline (no cache)
cargo test -p EdgeLorD --test cache_phase1_1_bench -- --ignored --features no_cache

# Cached
cargo test -p EdgeLorD --test cache_phase1_1_bench -- --ignored
```

Both runs use identical scenario scripts; output goes to separate CSV files.

### 5.4 Test Execution Example

```bash
# Run all Phase 1.1 tests
cargo test -p EdgeLorD cache_phase1_1

# Run with metrics
cargo test -p EdgeLorD cache_phase1_1_bench -- --ignored --nocapture
# Produces: target/debug/edgelord_cache_metrics_*.csv, PHASE_1_1_REPORT.md
```

---

## 6. Rejection Criteria (Automatic Failure)

Phase 1.1 **fails acceptance** if any occur:

1. ❌ Cache key excludes `OptionsFingerprint` or `WorkspaceSnapshotHash` (or fallback)
2. ❌ Cache reuse occurs when any input differs (violates INV D-CACHE-2)
3. ❌ Stale diagnostics publish (older DV overwrites newer)
4. ❌ Determinism test fails (same inputs → different outputs)
5. ❌ Metrics not produced (no CSV, no before/after numbers)
6. ❌ Any test in `cache_phase1_1_invariants.rs` or `cache_phase1_1_races.rs` fails

---

## 7. Deliverables Checklist

- [x] Cache key structure with all required components
- [x] Conservative fallback for incomplete SniperDB APIs
- [x] `ModuleCache` implementation with deterministic keying
- [x] Invariants tests: D-CACHE-1 (purity), D-CACHE-2 (sound reuse), D-CACHE-3 (monotone invalidation)
- [x] Race-condition tests preserving single-flight + no stale output
- [x] Metrics harness (CSV + report)
- [x] Acceptance threshold validation
- [x] Documentation: "how to run" + "what success looks like"

---

## 8. Files to Create/Modify

| File | Purpose | Status |
|------|---------|--------|
| `src/caching.rs` (new) | `ModuleCache` implementation | To implement |
| `src/lsp.rs` | Integrate cache into compilation flow | Modify |
| `src/proof_session.rs` | Wire cache into `did_open` / `did_change` | Modify |
| `tests/cache_phase1_1_invariants.rs` (new) | Invariants tests | To implement |
| `tests/cache_phase1_1_races.rs` (new) | Race-condition tests | To implement |
| `tests/cache_phase1_1_bench.rs` (new) | Metrics harness + scenarios | To implement |
| `Cargo.toml` | Add `no_cache` feature flag (optional) | Modify |

---

## 9. Success Criteria Summary

✅ **Phase 1.1 is done when:**

1. All invariants pass (D-CACHE-1, D-CACHE-2, D-CACHE-3)
2. No stale diagnostics under any DV ordering
3. Cache key includes all required components (options, workspace, unit, content, deps)
4. Metrics produced: CSV with P50/P95, hit rate, compilations per 100 keystrokes
5. Before/after comparison shows:
   - ≥ 60% cache hit rate in hot loop, OR
   - ≥ 25% reduction in compilations per 100 keystrokes, OR
   - ≥ 20% reduction in P95 latency
6. All test files compile and pass
7. Report clearly documents methodology and results

---

**Implementation Ready:** This spec is sufficient for an agent to implement Phase 1.1 without ambiguity.

**House Style:** This uses INV D-* naming consistent with Mac Lane invariant framework. Can be cross-referenced with broader INV D-, INV E-, INV T-* contracts if needed.
