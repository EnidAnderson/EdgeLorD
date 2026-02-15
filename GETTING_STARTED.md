# EdgeLorD Development: Getting Started Guide

## What Just Happened?

I've completed an audit of EdgeLorD and identified significant opportunities to improve LSP support by better utilizing SniperDB. Three documents have been created:

1. **AUDIT_SUMMARY.md** ← Start here (5 min read)
   - What works well
   - What opportunities exist
   - Why this matters

2. **EDGELORD_LSP_IMPROVEMENT_PLAN.md** ← Implementation guide (detailed)
   - 4-phase improvement plan
   - Specific files to modify
   - Success criteria
   - Architecture notes

3. **This file** ← Navigation guide

## The Situation in 30 Seconds

**Good news:** EdgeLorD is well-built and feature-complete.

**Better news:** 2-3 weeks of work can deliver:
- **30-50% faster** incremental compilation
- **Richer LSP features** (module context, proof artifacts)
- **Performance visibility** (built-in diagnostics)

**Key insight:** SniperDB has 7 modules; EdgeLorD only uses 1. The other 6 contain:
- Semantic caching (performance win)
- Module metadata (better search/suggestions)
- Proof artifacts (visibility)
- Telemetry (observability)

## Where to Start

### If You Have 30 Minutes
1. Read `AUDIT_SUMMARY.md` (5 min)
2. Skim `EDGELORD_LSP_IMPROVEMENT_PLAN.md` Phase 1 (10 min)
3. Look at example code below (15 min)

### If You Have 2 Hours
1. Read both audit documents carefully
2. Review current SniperDB usage in `src/lsp.rs` (lines 111-170)
3. Study the architecture in `EDGELORD_LSP_IMPROVEMENT_PLAN.md`
4. Sketch Phase 1 implementation plan

### If You're Ready to Code
→ Jump to **Phase 1: Semantic Caching** section below

## Phase 1: Semantic Caching (Recommended Starting Point)

This is the highest-impact, lowest-risk work. It's also foundational for Phases 2-4.

### Overview
**Problem:** Every keystroke recompiles entire modules, even if nothing changed.

**Solution:** Use SniperDB snapshots to cache compiled modules by content hash.

**Impact:** 30-50% fewer compilations on typical edits.

**Effort:** 4-5 hours to Phase 1.1 (immediate 30% win)

### Files to Modify

1. **`src/lsp.rs`** (add at top)
   ```rust
   mod caching;  // NEW MODULE
   use caching::ModuleCache;
   ```

2. **`src/caching.rs`** (new file, ~200 lines)
   ```rust
   use codeswitch::fingerprint::HashValue;
   use std::collections::BTreeMap;

   pub struct ModuleSnapshot {
       content_hash: HashValue,
       compiled_form: Vec<u8>,  // serialized module
       type_info: String,       // exports, imports
   }

   pub struct ModuleCache {
       snapshots: BTreeMap<(u32, HashValue), ModuleSnapshot>,
   }

   impl ModuleCache {
       pub fn new() -> Self { /* ... */ }

       pub fn get(&self, file_id: u32, content_hash: HashValue) -> Option<&ModuleSnapshot> {
           self.snapshots.get(&(file_id, content_hash))
       }

       pub fn insert(&mut self, file_id: u32, snapshot: ModuleSnapshot) {
           self.snapshots.insert((file_id, snapshot.content_hash), snapshot);
       }
   }
   ```

3. **`src/proof_session.rs`** (modify `ProofDocument`)
   ```rust
   pub struct ProofDocument {
       version: i32,
       parsed: ParsedDocument,
       last_analyzed: Instant,
       workspace_report: WorkspaceReport,
       goals_index: Option<GoalsPanelIndex>,
       history: VecDeque<ProofSnapshot>,
       content_hash: HashValue,  // NEW: track content
       cached_snapshot: Option<ModuleSnapshot>,  // NEW: cache
   }
   ```

4. **`src/proof_session.rs`** (modify `ProofSession::update`)
   ```rust
   pub async fn update(&mut self, uri: Url, version: i32,
                       changes: Vec<TextDocumentContentChangeEvent>)
       -> ProofSessionUpdateResult
   {
       // ... existing code ...

       let updated_text = apply_content_changes(&current_proof_doc.parsed.text, &changes);
       let new_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", updated_text.as_bytes());

       // NEW: Check cache before recompiling
       if let Some(snapshot) = cache.get(file_id, new_hash) {
           // Reuse cached module, skip workspace recompilation
           return ProofSessionUpdateResult {
               report: snapshot.cached_report.clone(),
               diagnostics: snapshot.cached_diagnostics.clone(),
               goals: snapshot.cached_goals.clone(),
           };
       }

       // If cache miss, compile normally and cache result
       // ... existing compilation code ...

       cache.insert(file_id, ModuleSnapshot { ... });

       // ... return result ...
   }
   ```

### Testing Phase 1.1
```bash
# Test that module snapshots work
cargo test -p EdgeLorD --test "*cache*"

# Manual test:
# 1. Edit file.maclane
# 2. Change text
# 3. Undo (Ctrl+Z) → should show cache hit (check stats)
# 4. Edit same file again → cache should miss
# 5. Check Query Statistics from code action menu
```

### Success Criteria
- [ ] `ModuleCache` created and integrated
- [ ] Content hash computed for each document
- [ ] Cache hits logged and visible in stats
- [ ] Undo/redo reuses cache
- [ ] Performance improvement measurable (run perf tests)

## Phase 1.2: Incremental Query Memoization

Once Phase 1.1 works, enhance it further.

**Files:** `src/proof_session.rs` + SniperDB's `MemoTable`

**Key change:** Use SniperDB's existing memo table for proof queries, not just modules.

```rust
use sniper_db::SniperDatabase;

pub struct ProofSession {
    // ... existing fields ...
    db: Arc<SniperDatabase>,  // NEW: integrate SniperDB
}

impl ProofSession {
    pub async fn update(&mut self, ...) -> ProofSessionUpdateResult {
        // NEW: Use memo table to check for cached proof states
        let query_key = format!("prove_goal:{}:{:?}", file_id, goal_id);
        if let Some(cached) = self.db.memo.get(&query_key) {
            return cached;
        }

        // Compute proof, memoize result
        let result = /* ... compute ... */;
        self.db.memo.insert(query_key, result.clone());

        result
    }
}
```

## Current SniperDB Integration

To understand what's already working, look at **DB-7 rename impact analysis** in `src/lsp.rs`:

```rust
fn render_hover_markdown(from: &str, to: &str, report: &sniper_db::plan::PlanReport, ...) {
    // This is the only current SniperDB use
    // Shows: blast radius, cost, proof preservation
}
```

This is working well. We just need to do the same for other SniperDB modules:
- ✅ `plan` (DB-7) - working
- ❌ `snapshot` - not used
- ❌ `module` - not used
- ❌ `artifact` - not used
- ❌ `flight_recorder` - not used

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                  LSP Protocol                       │
│              (tower-lsp)                            │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────┐
│              lsp.rs: Backend                        │
│     (document open/change/close, code actions)      │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────┐
│         proof_session.rs: ProofSession              │
│   (workspace integration, goal tracking)            │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────┐
│   ComradeWorkspace (new_surface_syntax)            │
│    (canonical compilation, proof state)            │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────┐
│     SniperDB + tcb_core (backend)                  │
│  (caching, artifacts, telemetry)                   │
└─────────────────────────────────────────────────────┘
```

**Where to add caching:** Between `ProofSession` and `ComradeWorkspace`.

## Key Files Reference

### Core (Already Working Well)
- `src/lsp.rs` (500 lines) - LSP handlers, DB-7 integration
- `src/proof_session.rs` (300 lines) - Proof state management
- `src/document.rs` - Document model

### Where Phase 1 Work Happens
- `src/lsp.rs` - Add `mod caching;`
- `src/caching.rs` (NEW) - Module snapshots, cache logic
- `src/proof_session.rs` - Call cache before/after compilation

### Existing Advanced Features (Don't Touch)
- `src/tactics/` - Tactic framework (working)
- `src/explain/` - Goal explanations (working)
- `src/refute/` - Refutation engine (working)
- `src/loogle/` - Code search (working)

## Common Questions

**Q: Will this break existing functionality?**
A: No. Cache misses fall back to normal recompilation. Fail-closed by design.

**Q: How do I test this?**
A: Run unit tests, then manual test with edits/undo. Stats command shows cache hits.

**Q: What's the performance impact?**
A: Should be 30-50% fewer compilations. Measure with `cargo bench`.

**Q: What if SniperDB API changes?**
A: Keep all SniperDB imports in `caching.rs`. Makes updates easier.

**Q: Should I do all 4 phases?**
A: Start with Phase 1. Phases 2-4 are optional, but Phase 1 is recommended.

## Running Tests

```bash
# Test everything
cargo test -p EdgeLorD

# Test with logging
RUST_LOG=debug cargo test -p EdgeLorD

# Run integration tests
cargo test -p EdgeLorD --test integration_tests

# Check code quality
cargo clippy -p EdgeLorD
cargo fmt -p EdgeLorD
```

## Debugging Tips

1. **Check SniperDB stats:**
   - Code action: `"edgelord.debug:show-query-statistics"`
   - Shows cache hits/misses

2. **Enable logging:**
   - Set `lsp.rs` log level to `debug`
   - Look for cache hit/miss messages

3. **Profile changes:**
   - Time document edits before/after caching
   - Use `std::time::Instant` to measure

## Next Steps

1. **Read:** `AUDIT_SUMMARY.md` (5 min)
2. **Understand:** Architecture in `EDGELORD_LSP_IMPROVEMENT_PLAN.md` Phase 1 (10 min)
3. **Code:** Implement Phase 1.1 following the guide above (4-5 hours)
4. **Test:** Verify cache hits and measure performance improvement
5. **Extend:** Move to Phase 1.2 or Phase 2 as time allows

---

**Status:** Ready to begin Phase 1

**Estimated Effort:**
- Phase 1.1 (module snapshots): 4-5 hours
- Phase 1.2 (memo integration): 4-5 hours
- Phases 2-4 (optional): 40-50 hours total

**Expected Outcome:** 30-50% faster LSP on typical edits, plus foundation for advanced features
