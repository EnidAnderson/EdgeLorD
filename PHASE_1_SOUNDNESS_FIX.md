# Phase 1 Soundness Fix: Complete Cache Keys

**Date**: February 8, 2026
**Status**: ✅ FIXED - Restored sound semantics
**Severity**: Critical correctness issue (silent nondeterminism)

---

## The Bug (Now Fixed)

### What Was Wrong

The original Phase 1 implementation cached `WorkspaceReport` (compilation output) using only a **2-component key**:
```rust
(file_id, content_hash)  // INCOMPLETE KEY ❌
```

This violated **INV D-CACHE-2 (Sound reuse)** from Phase 1.1:
> Only reuse when all compilation inputs are identical.

### Why This Is Unsound

A `WorkspaceReport` depends on **at least** these inputs:
- ✅ File content (we hashed this)
- ❌ Compile options (we DIDN'T check)
- ❌ Transitive dependencies (we DIDN'T check)

**Concrete failure scenario**:
```
FileA.mc imports FileB.mc

1. Compile both → cache both
2. Edit FileB.mc (change its content)
3. Try to compile FileA (content unchanged)
4. Phase 1 cache HIT: returns cached WorkspaceReport for A
5. BUG: A's output is STALE (B changed, A depends on B!)
```

This is **silent nondeterminism** - the kind that causes subtle, hard-to-debug failures in production.

---

## The Fix: Complete 4-Component Keys

### What Changed

**ModuleSnapshot now uses a COMPLETE key that includes ALL compilation inputs:**

```rust
pub struct ModuleSnapshot {
    // Cache key components (must ALL match for reuse)
    pub file_id: u32,                           // Which file
    pub content_hash: HashValue,                // File's byte content
    pub options_fingerprint: HashValue,         // Compile options/flags
    pub dependency_fingerprint: HashValue,      // Transitive deps hash

    // Cached output (only valid when key matches)
    pub report: WorkspaceReport,
    pub diagnostics: Vec<Diagnostic>,
    pub timestamp: SystemTime,
}
```

**Cache lookup now requires all 4 components to match:**

```rust
if let Some(snapshot) = snapshot_cache.get(
    file_id,                     // Same file?
    content_hash,                // Same content?
    options_fingerprint,         // Same options?
    dependency_fingerprint       // Same deps?
) {
    // Safe to reuse - all inputs match
    return snapshot.report;
}
```

### Invariants Enforced

**INV PHASE-1-MODULE-1 (Sound Reuse)**
```
Same (file_id, content_hash, options_fingerprint, dependency_fingerprint)
  → Safe to reuse cached WorkspaceReport
```

**INV PHASE-1-MODULE-2 (Content Stability)**
```
Different content_hash → cache miss (file changed)
```

**INV PHASE-1-MODULE-3 (Dependency Sensitivity)**
```
Different dependency_fingerprint → cache miss (imports changed)
```

---

## Changes Made

### 1. ModuleSnapshot Structure (`src/caching.rs`)

**Before**:
```rust
pub struct ModuleSnapshot {
    pub file_id: u32,
    pub content_hash: HashValue,
    pub report: WorkspaceReport,
    pub diagnostics: Vec<Diagnostic>,
    pub timestamp: SystemTime,
}
```

**After**:
```rust
pub struct ModuleSnapshot {
    pub file_id: u32,
    pub content_hash: HashValue,
    pub options_fingerprint: HashValue,        // NEW
    pub dependency_fingerprint: HashValue,     // NEW
    pub report: WorkspaceReport,
    pub diagnostics: Vec<Diagnostic>,
    pub timestamp: SystemTime,
}
```

### 2. ModuleSnapshotCache Key (`src/caching.rs`)

**Before**:
```rust
cache: BTreeMap<(u32, HashValue), ModuleSnapshot>
```

**After**:
```rust
cache: BTreeMap<(u32, HashValue, HashValue, HashValue), ModuleSnapshot>
```

### 3. Cache Methods (`src/caching.rs`)

**Before**:
```rust
pub fn get(&mut self, file_id: u32, content_hash: HashValue) -> Option<ModuleSnapshot>
pub fn insert(&mut self, file_id: u32, snapshot: ModuleSnapshot)
```

**After**:
```rust
pub fn get(
    &mut self,
    file_id: u32,
    content_hash: HashValue,
    options_fingerprint: HashValue,
    dependency_fingerprint: HashValue,
) -> Option<ModuleSnapshot>

pub fn insert(&mut self, snapshot: ModuleSnapshot)
```

### 4. ProofSession Integration (`src/proof_session.rs`)

**Before** (unsound - returns stale data):
```rust
let unit_content_hash = ...;
let file_id = ...;

if let Some(snapshot) = snapshot_cache.get(file_id, unit_content_hash) {
    return ProofSessionUpdateResult { report: snapshot.report };  // ❌ STALE!
}
```

**After** (sound - checks all inputs):
```rust
let unit_content_hash = ...;
let options_fingerprint = ...;
let workspace_snapshot_hash = ...;
let file_id = ...;

if let Some(snapshot) = snapshot_cache.get(
    file_id,
    unit_content_hash,
    options_fingerprint,
    workspace_snapshot_hash,  // Conservative: includes all deps
) {
    return ProofSessionUpdateResult { report: snapshot.report };  // ✅ SAFE
}
```

### 5. Critical Tests Added (`src/caching.rs`)

**New unit test: test_module_snapshot_cache_miss_on_deps_change**
```rust
#[test]
fn test_module_snapshot_cache_miss_on_deps_change() {
    // SOUNDNESS TEST: Different dependencies → cache miss
    // This test would FAIL with the old buggy (file_id, content_hash) key

    let snapshot = ModuleSnapshot {
        file_id,
        content_hash,
        options_fingerprint,
        dependency_fingerprint: deps_fp_state1,  // State 1
        report: WorkspaceReport::default(),
        ...
    };
    cache.insert(snapshot);

    // Now deps change
    let retrieved = cache.get(
        file_id,
        content_hash,
        options_fingerprint,
        deps_fp_state2,  // DIFFERENT - must miss
    );

    assert!(retrieved.is_none(), "Must miss when deps change");
}
```

**New unit test: test_module_snapshot_cache_miss_on_options_change**
```rust
#[test]
fn test_module_snapshot_cache_miss_on_options_change() {
    // SOUNDNESS TEST: Different compile options → cache miss
    // (Same pattern for options fingerprint)
}
```

---

## Verification

### How the Fix Prevents the Bug

**Original scenario (now FIXED)**:
```
1. Compile FileA (depends on FileB): cache with (file_a, hash_a, opts, deps_fp_state1)
2. Edit FileB (change its content)
3. Try to compile FileA (content unchanged):
   - Query cache: get(file_a, hash_a, opts, deps_fp_state2)  ← deps_fp changed
   - Cache miss ✅ (old code would hit and serve stale data)
   - Recompile FileA correctly
```

### Tests That Now Catch the Bug

These tests **would FAIL** with the old (file_id, content_hash) key:
- `test_module_snapshot_cache_miss_on_deps_change` - caught by INV PHASE-1-MODULE-3
- `test_module_snapshot_cache_miss_on_options_change` - new invariant enforcement

Run with:
```bash
cd EdgeLorD && cargo test --lib caching::tests::test_module_snapshot_cache_miss_on_deps_change -- --nocapture
```

---

## Design Rationale

### Why Complete Keys (Not Restricted Caching)

We chose **Option B** (complete keys) over **Option A** (restrict to safe stages):

| Aspect | Option A (Restrict) | Option B (Complete) |
|--------|-------------------|-------------------|
| **Safety** | Depends on architecture knowledge | Obviously safe (complete key) |
| **Clarity** | "What can we cache?" question | Clear: anything with complete key |
| **Maintenance** | Easy to accidentally violate | Hard to violate (structural) |
| **Performance** | Limited scope | Full scope (module outputs) |

**Decision**: Option B is production-ready and correct by construction.

### Why Not Persist to SniperDB Yet?

The code threads SniperDatabase into ProofSession but doesn't persist snapshots to it yet.

**Reason**: Phase 1 focuses on correctness. SniperDB persistence can follow once soundness is verified.

**Deferred to Phase 1.2**: Serialize ModuleSnapshot to SniperDB for cross-session reuse.

---

## Impact Assessment

### What This Fixes
- ✅ Eliminates silent nondeterminism (stale cached outputs)
- ✅ Restores INV D-CACHE-2 compliance
- ✅ Makes Phase 1 cache sound and production-safe

### What This Doesn't Change
- ✅ Phase 1.1 cache still works as designed
- ✅ Statistics dashboard still reports hits/misses
- ✅ LSP command interface unchanged
- ✅ Performance characteristics similar (complete key is small)

### Compilation Status
- ✅ `cargo check --lib` passes
- ✅ All unit tests compile and pass
- ✅ All integration tests compile and pass
- ✅ No regressions in Phase 1.1 code

---

## Summary

**Phase 1 is now sound and production-ready.**

The fix ensures that **only when all compilation inputs match** do we reuse cached WorkspaceReports. This eliminates the silent nondeterminism that was lurking in the original 2-component key design.

The key insight: A macro cache key (file + content) is insufficient when that output depends on workspace context. The safe approach is to include all context in the key, making reuse explicit and correct.

**Next step**: Run benchmarks to verify cache efficiency with the complete keys.
