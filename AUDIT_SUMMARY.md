# EdgeLorD LSP Audit Summary (2026-02-08)

## Overview

EdgeLorD has a **solid, feature-rich foundation** but is **underutilizing SniperDB** capabilities. The LSP server successfully integrates with ComradeWorkspace for canonical compilation and has advanced features like DB-7 rename impact analysis, but leaves significant performance and UX improvements on the table.

## Audit Findings

### Strengths ✅

1. **Clean Architecture**
   - Clear separation: LSP protocol → Proof Session → ComradeWorkspace
   - LSP handlers properly delegate to session/workspace
   - Configuration system well-structured
   - Error handling fail-closed (safe defaults)

2. **Advanced Features Already Implemented**
   - Debounce + single-flight pattern (prevents stale diagnostics)
   - Goals panel with stable anchors
   - Proof snapshots (up to 10 history points)
   - Tactic framework with registry-based dispatch
   - Explanation system (for blocked/inconsistent goals)
   - Refutation engine with multiple probes
   - Loogle workspace search
   - Semantic token highlighting
   - DB-7 hover for rename impact (blast radius, cost, proof preservation)

3. **Core Infrastructure Sound**
   - ComradeWorkspace integration correct (per HANDOFF.md)
   - Workspace diagnostics properly consumed
   - Document lifecycle properly managed
   - No "shadow streams" (diagnostics flow through canonical path)

### Opportunities 🎯

1. **Semantic Caching** (Biggest Win)
   - Currently: Every keystroke recompiles entire modules
   - Possible: Use SniperDB snapshots to skip recompilation for unchanged modules
   - Expected: 30-50% fewer compilations on typical edits
   - Effort: 4-5 hours
   - Status: **No module-level caching exists**

2. **Cross-File Analysis**
   - Currently: Module metadata queried on-demand
   - Possible: Maintain workspace module index from SniperDB
   - Benefit: Faster imports, better search, smarter suggestions
   - Effort: 4-5 hours
   - Status: **Partial** (Loogle indexer works, but not connected to module metadata)

3. **Proof Artifact Integration**
   - Currently: Proof snapshots created but not used
   - Possible: Connect blob store to goals panel, show proof status
   - Benefit: Proof preservation visible, can navigate history
   - Effort: 4 hours
   - Status: **Missing** (snapshots exist but artifacts not displayed)

4. **Performance Observability**
   - Currently: No metrics on LSP performance
   - Possible: Integrate SniperDB flight recorder for telemetry
   - Benefit: User sees performance data, can identify bottlenecks
   - Effort: 4 hours
   - Status: **Not integrated**

5. **Enhanced Hover & Code Actions**
   - Currently: Hover shows types, code actions generic
   - Possible: Add module origin, usage count, smart tactic ranking
   - Benefit: Richer context, better suggestions
   - Effort: 6 hours
   - Status: **Partial** (DB-7 works, general features basic)

### Gaps Found 🚨

1. **SniperDB Modules Unused:**
   - ❌ `snapshot` - Module snapshots not used for caching
   - ❌ `module` - Module metadata API not integrated
   - ❌ `ops` - General operations API not used
   - ❌ `artifact` - Artifact tracking in goals not implemented
   - ❌ `edit` - Edit scheduling not leveraged
   - ❌ `flight_recorder` - No telemetry integration
   - ⚠️ `store` - Only default memory store
   - ✅ `plan` - DB-7 integration working well

2. **Performance Issues:**
   - No query memoization between edits
   - No module-level caching
   - Workspace index rebuilt on each change
   - No performance metrics/warnings

3. **Proof Integration Incomplete:**
   - Snapshots created but not displayed
   - No proof history UI
   - No artifact linking in goals panel
   - No proof preservation warnings in diagnostics

4. **Code Action Limitations:**
   - External commands don't feed into LSP quick fixes
   - Tactics not ranked by applicability
   - No import suggestions from search

## What This Means

**Good news:** EdgeLorD doesn't need major refactoring. The foundation is sound.

**Opportunity:** 2-3 weeks of focused work can:
- **30-50% faster** incremental compilation (semantic caching)
- **Richer LSP features** (module context, proof artifacts)
- **Performance visibility** (flight recorder integration)
- **Better diagnostics** (proof preservation warnings)

## Recommended Next Steps

### Immediate (This Week)
1. **Start Phase 1: Semantic Caching**
   - Implement module snapshots (4-5 hours)
   - Get incremental memos working (4-5 hours)
   - Add query stats dashboard (2-3 hours)
   - Should see immediate 30%+ performance improvement

### Short Term (Next Week)
2. **Phase 2: Proof Artifact Integration**
   - Show proof status in goals panel
   - Implement proof history navigation
   - Add preservation loss warnings

3. **Phase 3: Enhanced LSP Features**
   - Smart hover with module info
   - Code actions ranked by applicability
   - Module-aware Loogle

### Medium Term
4. **Phase 4: Observability**
   - Flight recorder integration
   - Performance dashboard
   - Build time tracking

## Files Reviewed

### Core LSP
- ✅ `src/lsp.rs` (500+ lines) - LSP protocol, DB-7 integration
- ✅ `src/proof_session.rs` (300+ lines) - Proof state, workspace integration
- ✅ `src/document.rs` - Document model

### Advanced Features
- ✅ `src/explain/` - Goal explanations
- ✅ `src/refute/` - Refutation tactics
- ✅ `src/tactics/` - Tactic framework
- ✅ `src/loogle/` - Code search
- ✅ `src/highlight.rs` - Semantic tokens

### SniperDB Integration
- ✅ `src/lsp.rs` lines 111-170 - DB-7 hover (only SniperDB use)
- ❌ All other SniperDB modules unused from LSP

## Conclusion

**EdgeLorD is 80% of the way there.** It has excellent infrastructure and advanced features. The remaining 20% is integrating SniperDB's capabilities for:
1. Performance (semantic caching)
2. Context (module metadata)
3. Visibility (proof artifacts, flight recorder)

**Implementation difficulty:** Low-to-Medium (no architectural changes needed)

**Expected impact:** High (significant performance + UX improvements)

**Timeline:** 2-3 weeks for full implementation

---

**Next Action:** Review detailed improvement plan in `EDGELORD_LSP_IMPROVEMENT_PLAN.md` and start Phase 1 (semantic caching).
