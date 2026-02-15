# EdgeLorD LSP Improvement: Complete Package

**Status:** Ready for implementation
**Structure:** Tiered documentation (summary → architecture → spec → implementation guide)
**Key Principle:** "Dredging is a feature"—attach LSP to DB surfaces that expose kernel invariants, don't invent new UI

---

## Document Map

### 1. **AUDIT_SUMMARY.md** (5 min read)
   - **Audience:** Decision makers, team leads
   - **Content:** What works, opportunities, gaps, why this matters
   - **Key Insight:** EdgeLorD is 80% complete; SniperDB has 7 modules, only 1 is used
   - **Takeaway:** 2-3 weeks of focused work → 30-50% performance improvement + richer features

### 2. **EDGELORD_LSP_IMPROVEMENT_PLAN.md** (Architecture vision)
   - **Audience:** Architects, long-term planners
   - **Content:** 4-phase roadmap (semantic caching → proof artifacts → enhanced features → observability)
   - **Key Insight:** Each phase integrates deeper into SniperDB to expose invariants
   - **Takeaway:** Phase 1 is foundation; Phases 2-4 build on it

### 3. **PHASE_1_1_ACCEPTANCE_SPEC.md** (Implementation contract)
   - **Audience:** Implementation agent
   - **Content:** Hard specifications, invariants, test cases, rejection criteria
   - **Structure:**
     - Definitions (CacheKey structure)
     - Cache key construction (all 5 required fields)
     - Three invariants (INV D-CACHE-1/2/3) with test cases each
     - Race-condition guard (preserves single-flight + no stale output)
     - Measurement methodology (CSV + before/after metrics)
     - Rejection criteria (automatic failure conditions)
     - Deliverables checklist
   - **Key Principle:** "Correctness first, performance second"
   - **Binding:** This is your contract. Cannot ship without meeting all criteria.

### 4. **README_PHASE_1_1.md** (Implementation guide)
   - **Audience:** Implementation agent (practical how-to)
   - **Content:**
     - TL;DR of what to build
     - Cache key structure (Rust code template)
     - Integration points in codebase
     - Architecture diagram
     - Pitfalls to avoid
     - Testing strategy (invariants, races, benchmarks)
     - Success criteria checklist
     - FAQ
   - **Takeaway:** Everything you need to start coding

### 5. **GETTING_STARTED.md** (Optional reference)
   - Earlier high-level guide with code examples
   - Still useful for understanding the architecture

---

## The Strategy (Why This Order, Why This Structure)

### The Problem
EdgeLorD works well but recompiles on every keystroke, causing latency spikes. SniperDB has caching infrastructure; EdgeLorD doesn't use it.

### The Approach: "Dredging"
Instead of inventing new UI features, **attach LSP to DB surfaces that expose kernel invariants.** This approach:
1. **Validates the DB** (forces real-world usage of SniperDB modules)
2. **Surfaces hidden bugs** (if compilation isn't a pure function, caching reveals it)
3. **Delivers real value** (users see immediate performance improvement)
4. **Unblocks future work** (Phase 2-4 depend on Phase 1 correctness)

### Why This Order?
```
Phase 1 (Semantic Caching)
  └─ Forces determinism, exposes impurity bugs
  └─ Foundation for everything else

Phase 2 (Proof Artifacts)
  └─ Depends on Phase 1 snapshots
  └─ Uses blob store to surface invariants

Phase 3 (Enhanced LSP)
  └─ Depends on Phase 2 metadata/artifacts
  └─ Provides richer context

Phase 4 (Observability)
  └─ Can run parallel, depends on flight recorder API
  └─ Completes the picture
```

### Why This Documentation Structure?
1. **AUDIT_SUMMARY** → Align on goal
2. **EDGELORD_LSP_IMPROVEMENT_PLAN** → Understand long-term vision
3. **PHASE_1_1_ACCEPTANCE_SPEC** → Lock down requirements (no ambiguity)
4. **README_PHASE_1_1** → Practical implementation guide
5. **This INDEX** → Navigate and understand the whole package

---

## Core Principles

### 1. Correctness First
- Invariants are non-negotiable
- Better to be conservative (over-invalidate) than wrong
- Metrics without correctness don't count

### 2. Determinism is the North Star
- Same inputs → identical outputs, always
- Test this explicitly (INV D-CACHE-1)
- If determinism fails, it's a core bug, not a caching issue

### 3. No Single-Flight Violations
- Cache lookup must happen inside the gate
- Stale diagnostics are automatic rejection
- Test this explicitly (race condition tests)

### 4. Measurement Over Guesses
- "30-50% improvement" is a hypothesis
- CSV + before/after numbers are evidence
- Target thresholds are acceptance criteria

### 5. Conservative Fallback
- If SniperDB API missing → degrade safely (all-invalidate)
- Never assume the API exists
- Fallback is tested, not untested code path

---

## Quick Start (For Implementation Agent)

1. **Read in order:**
   ```
   AUDIT_SUMMARY.md (5 min)
   ↓
   PHASE_1_1_ACCEPTANCE_SPEC.md (sections 0-3, 15 min)
   ↓
   README_PHASE_1_1.md (30 min)
   ```

2. **Understand the contract:**
   - CacheKey must have all 5 fields (options, workspace, unit, content, deps)
   - INV D-CACHE-1 (purity), INV D-CACHE-2 (sound reuse), INV D-CACHE-3 (monotone invalidation)
   - INV D-RACE-1 (no stale diagnostics)
   - Measurement must produce CSV + report

3. **Implement:**
   - `src/caching.rs` (ModuleCache impl)
   - Integration in `src/lsp.rs`, `src/proof_session.rs`
   - Tests: invariants, races, benchmarks

4. **Validate:**
   - All tests pass
   - CSV metrics show ≥60% hit rate OR ≥25% compilation reduction OR ≥20% latency improvement
   - No correctness regressions

---

## What "Done" Looks Like

✅ Phase 1.1 complete when:
- All three invariants (D-CACHE-1/2/3) verified by tests
- Race conditions prevented (single-flight gate intact)
- Metrics show measurable improvement (CSV + report)
- Code review approved (correctness + implementation quality)

📦 Deliverables:
- `src/caching.rs` (ModuleCache)
- `tests/cache_phase1_1_invariants.rs`
- `tests/cache_phase1_1_races.rs`
- `tests/cache_phase1_1_bench.rs`
- `PHASE_1_1_REPORT.md` (metrics + analysis)

---

## Why This Matters (Architectural Perspective)

This isn't "performance optimization." It's **making the compilation pipeline provably pure and deterministic.**

When caching works:
- ✅ Compilation is a function of (inputs) → (outputs)
- ✅ Cache can work correctly
- ✅ Diagnostics are deterministic
- ✅ Future optimizations (Phase 2-4) can trust the foundation

When caching fails (bugs in core):
- ❌ Compilation has hidden state dependencies
- ❌ Diagnostics are non-deterministic
- ❌ Future optimizations will fail too
- ❌ Core invariants violated

**Phase 1.1 is the canary in the coal mine.** If it fails, we've found a core bug. If it succeeds, we've proven the system is solid.

---

## References

- **SniperDB Design:** `clean_kernel/crates/sniper_db/src/lib.rs`
- **Existing DB-7 Use:** `src/lsp.rs` lines 111-170
- **ProofSession:** `src/proof_session.rs` lines 56-220
- **ComradeWorkspace:** `clean_kernel/crates/new_surface_syntax/src/comrade_workspace.rs`

---

## Next Action

**For implementation agent:**
→ Read PHASE_1_1_ACCEPTANCE_SPEC.md, then README_PHASE_1_1.md, then start coding `src/caching.rs`

**For reviewer:**
→ Read AUDIT_SUMMARY.md + PHASE_1_1_ACCEPTANCE_SPEC.md (sections 0-3)

**For lead architect:**
→ Review complete package; suggest tightening if needed

---

**Package Created:** 2026-02-08
**Status:** Implementation-ready
**Effort:** 4-5 hours (code) + 2-3 hours (tests/metrics)
**Expected Outcome:** Solid, proven foundation for LSP improvements
