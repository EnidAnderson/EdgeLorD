# Phase 1.1 Implementation Handoff

**Status:** Ready for implementation
**Target Audience:** Implementation agent (Claude)
**Estimated Effort:** 4-5 hours (core), +2-3 hours (tests/metrics)
**Expected Outcome:** 30%+ performance improvement with hard correctness guarantees

---

## What You're Building

A **cache for compiled modules** that reuses computation when file content and compilation options haven't changed. This is the foundation for all later LSP improvements (proof artifacts, observability, enhanced features).

Key insight: **This isn't optional UI polish—it's a correctness infrastructure play.** It forces the entire compilation pipeline to behave like a deterministic function, which uncovers bugs in core systems.

---

## Documents (Read in This Order)

1. **AUDIT_SUMMARY.md** (5 min)
   - What's working in EdgeLorD
   - What opportunities exist
   - Why SniperDB integration matters

2. **EDGELORD_LSP_IMPROVEMENT_PLAN.md** (15 min, Phase 1 section)
   - The original architecture vision
   - 4-phase roadmap
   - Why Phase 1 is foundational

3. **PHASE_1_1_ACCEPTANCE_SPEC.md** (THIS IS YOUR CONTRACT)
   - Hard definitions (cache key structure, invariants)
   - Three non-negotiable invariants with test cases
   - Race-condition guard (preserves single-flight)
   - Measurement methodology (real numbers, not guesses)
   - Rejection criteria (automatic failure conditions)

**Key difference:** This spec is tight. No ambiguity. You cannot accidentally ship a correctness bug.

---

## TL;DR: What to Implement

### 1. Cache Key Structure (Mandatory)

```rust
pub struct CacheKey {
    options_fingerprint: HashValue,      // hash of compile options
    workspace_snapshot_hash: HashValue,  // hash of workspace state
    unit_id: UnitId,                     // file_id or module_id
    unit_content_hash: HashValue,        // hash of file content
    dependency_fingerprint: HashValue,   // hash of transitive deps
}
```

**Important:** All five fields are required. Missing any one is automatic rejection.

### 2. Module Cache Type

```rust
pub struct ModuleCache {
    snapshots: BTreeMap<CacheKey, CacheValue>,
    stats: CacheStats,  // for metrics
}

pub struct CacheValue {
    workspace_report: WorkspaceReport,  // the compiled output
    diagnostics: Vec<Diagnostic>,       // for publishing
    timestamp: Instant,                 // for debugging
}

pub struct CacheStats {
    hits: u64,
    misses: u64,
    miss_reasons: BTreeMap<String, u64>,  // track why misses occurred
}
```

### 3. Integration Points

- **`src/lsp.rs`:** Add `mod caching;` at top
- **`src/caching.rs`:** New file with `ModuleCache` impl
- **`src/proof_session.rs`:** Call `cache.get(key)` before compile, `cache.insert()` after
- **`src/proof_session.rs`:** Maintain cache key inside single-flight gate

### 4. Three Invariants to Verify

- **INV D-CACHE-1 (Purity):** Same inputs → identical outputs
- **INV D-CACHE-2 (Sound Reuse):** Only reuse when key matches exactly
- **INV D-CACHE-3 (Monotone Invalidation):** Edits invalidate affected caches

### 5. Tests (Must Pass)

```
tests/cache_phase1_1_invariants.rs    // D-CACHE-1, D-CACHE-2, D-CACHE-3
tests/cache_phase1_1_races.rs         // Single-flight, no stale output
tests/cache_phase1_1_bench.rs         // Metrics: CSV + report
```

### 6. Metrics Output

Run scenarios and produce:
- `edgelord_cache_metrics_<timestamp>.csv` with detailed per-edit data
- `PHASE_1_1_REPORT.md` with before/after comparison
- Target: ≥60% cache hit rate OR ≥25% reduction in compilations OR ≥20% latency improvement

---

## Architecture Overview

```
┌────────────────────────────────────┐
│ LSP (didOpen/didChange/didClose)  │
└──────────────┬─────────────────────┘
               │
┌──────────────▼─────────────────────┐
│   ProofSession::update()           │
│  (single-flight gate: acquire)     │
├────────────────────────────────────┤
│  1. Compute CacheKey               │
│  2. cache.get(key)?                │ ← CACHE LOOKUP
│     └─ if hit: use cached output   │
│     └─ else: compile & cache       │
│  3. Publish diagnostics            │
│  (if DV still current)             │
└──────────────┬─────────────────────┘
               │
        (single-flight gate: release)
```

**Critical:** Cache lookup and publish must happen inside the same gate to prevent stale diagnostics.

---

## Fallback Strategy (If SniperDB APIs Incomplete)

If SniperDB doesn't expose full workspace snapshot or dependency API:

```rust
// Fallback: hash all open documents
WorkspaceSnapshotHash_fallback = hash_of_sorted_list([
    (doc_id, doc_content_hash) for each open doc
])

// Fallback: use workspace snapshot hash as dependency fingerprint
DependencyFingerprint_fallback = WorkspaceSnapshotHash_fallback
// Result: Conservative all-invalidate on any workspace change (safe, just less efficient)
```

This is acceptable for Phase 1.1. It's over-invalidating but correct.

---

## Pitfalls to Avoid

### ❌ Don't do this:

1. **Partial cache key:** `CacheKey = (file_id, content_hash)` only
   - Missing options → wrong reuse when options change
   - Missing workspace → wrong reuse when imports change
   - Automatic rejection

2. **Cache outside single-flight gate:**
   - Cache hit from old thread publishes over new DV → stale diagnostics
   - Automatic rejection

3. **No measurement:**
   - Claim "30-50% improvement" without CSV/numbers
   - Automatic rejection

4. **Skip fallback:**
   - Assume SniperDB API exists
   - Will panic/crash when API missing
   - Automatic rejection

### ✅ Do this instead:

1. **Compute safe key explicitly:**
   ```rust
   let options_hash = hash_compile_options(&options);
   let workspace_hash = hash_workspace_snapshot(&workspace);
   let key = CacheKey {
       options_fingerprint: options_hash,
       workspace_snapshot_hash: workspace_hash,
       ...
   };
   ```

2. **Cache inside gate:**
   ```rust
   let _guard = single_flight.acquire(unit_id, dv);  // acquire

   let key = compute_cache_key(...);
   let output = if let Some(cached) = cache.get(&key) {
       cached.clone()
   } else {
       let fresh = compile(...);
       cache.insert(key, fresh.clone());
       fresh
   };

   publish_if_current(dv, output);
   drop(_guard);  // release
   ```

3. **Always measure:**
   ```rust
   let start = Instant::now();
   let (output, cache_hit) = compile_or_hit(...);
   let elapsed = start.elapsed();

   stats.record(edgelord_cache_metrics {
       timestamp: now(),
       unit_id, dv,
       cache_hit,
       compile_ms: elapsed.as_millis(),
       ...
   });
   ```

4. **Use fallback:**
   ```rust
   let workspace_hash = sniper_db.workspace_snapshot()
       .unwrap_or_else(|_| compute_fallback_workspace_hash());
   ```

---

## Testing Strategy

### Run Invariants Tests (Must Pass)

```bash
cargo test -p EdgeLorD cache_phase1_1_invariants -- --nocapture
```

This verifies:
- Purity: Same inputs produce identical outputs
- Sound reuse: Wrong key never hits cache
- Monotone invalidation: Edits invalidate dependents

### Run Race Tests (Must Pass)

```bash
cargo test -p EdgeLorD cache_phase1_1_races -- --nocapture
```

This verifies:
- Single-flight gate prevents concurrent compiles
- Stale diagnostics never publish

### Run Benchmarks (Produces Metrics)

```bash
# Baseline (no cache)
cargo test -p EdgeLorD cache_phase1_1_bench -- --ignored --features no_cache

# Cached
cargo test -p EdgeLorD cache_phase1_1_bench -- --ignored

# Check outputs
cat target/debug/edgelord_cache_metrics_*.csv
cat PHASE_1_1_REPORT.md
```

### Manual Smoke Test

```bash
# Start EdgeLorD LSP
cargo run --bin edgelord

# In editor: open a file, make rapid edits
# Check: diagnostics appear quickly, no latency jumps
# Verify: cache_phase1_1_bench shows hit rate > 60%
```

---

## Success Criteria (Copy This Checklist)

**Phase 1.1 is done when ALL of these are true:**

- [ ] CacheKey has all five fields (options, workspace, unit, content, deps)
- [ ] Conservative fallback implemented (if SniperDB APIs incomplete)
- [ ] INV D-CACHE-1 test passes (purity)
- [ ] INV D-CACHE-2 test passes (sound reuse)
- [ ] INV D-CACHE-3 test passes (monotone invalidation)
- [ ] INV D-RACE-1 test passes (no stale diagnostics)
- [ ] CSV output produced with per-edit metrics
- [ ] PHASE_1_1_REPORT.md generated with before/after numbers
- [ ] Hit rate ≥60% OR compilations reduced ≥25% OR P95 latency reduced ≥20%
- [ ] All compilation in `cargo test` passes
- [ ] No clippy warnings or formatting issues
- [ ] Code review sign-off on cache correctness

---

## Files You'll Modify/Create

```
NEW:
  src/caching.rs                  // ModuleCache impl
  tests/cache_phase1_1_invariants.rs
  tests/cache_phase1_1_races.rs
  tests/cache_phase1_1_bench.rs

MODIFY:
  src/lsp.rs                      // Add mod caching; integrate cache
  src/proof_session.rs            // Call cache in did_open/did_change
  Cargo.toml                      // (optional) no_cache feature flag
```

---

## Questions to Resolve Before Starting

### Q: What's `UnitId`? Is it per-file or per-module?

**A:** For Phase 1.1, use **per-file** (file path or FileId). SniperDB tracks files; modules can come later.

### Q: What if SniperDB doesn't have a workspace snapshot API?

**A:** Use the fallback: hash all open documents in sorted order. It's over-conservative but safe.

### Q: Do I need to implement persistent caching (to disk)?

**A:** No. Phase 1.1 is in-memory only. `BTreeMap<CacheKey, CacheValue>`. Persistence can be Phase 1.2.

### Q: What about concurrency? Is the cache thread-safe?

**A:** You're already inside a single-flight gate. The cache can be shared via `Arc<Mutex<ModuleCache>>`. Lock is held only for get/insert, released before publish.

### Q: How do I know what "options" are?

**A:** Look at `ComradeWorkspace::did_open` signature. Whatever options it takes (OptionsId, facet, etc.), hash them.

### Q: What format for CSV? Can I use a library?

**A:** Any CSV format is fine. Use `csv` crate if available, or write it manually. Just needs columns: timestamp, unit, dv, cache_hit, compile_ms, total_ms, etc.

---

## Next Steps (After You're Ready to Code)

1. Read **PHASE_1_1_ACCEPTANCE_SPEC.md** carefully (this is your contract)
2. Create `src/caching.rs` with `ModuleCache` struct
3. Implement cache key computation (all five fields)
4. Integrate cache into `ProofSession::update()`
5. Write invariants tests (copy test cases from spec)
6. Write race tests (single-flight preservation)
7. Write benchmark harness (scenario scripts + metrics)
8. Run all tests; produce CSV + report
9. Compare before/after metrics
10. Submit with evidence

---

## Resources

- **SniperDB source:** `clean_kernel/crates/sniper_db/src/`
- **Existing DB-7 integration:** `src/lsp.rs` lines 111-170 (example of SniperDB use)
- **ProofSession current code:** `src/proof_session.rs` lines 56-220
- **ComradeWorkspace API:** `clean_kernel/crates/new_surface_syntax/src/comrade_workspace.rs`

---

## Final Thoughts

This is **correctness-first, performance-second.** The invariants tests are more important than hitting the performance targets. If you achieve all three invariants but hit rate is only 40%, that's a valid (if disappointing) Phase 1.1. But if you hit 80% cache hit rate and fail an invariant test, that's rejection.

The spec is intentionally tight so you **can't accidentally slip in a bug.** Use it as your checklist. When in doubt, ask "does this violate INV D-CACHE-1/2/3 or the race-condition guard?"

Good luck. This is well-scoped, well-motivated work. 🚀

---

**Owner:** Implementation agent (Claude)
**Reviewer:** Lead architect
**Status:** Ready to implement
**ETA:** 1 day (6-8 hours)
