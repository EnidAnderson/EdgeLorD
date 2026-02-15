# EdgeLorD LSP Improvement Plan (2026-02-08)

## Current State Assessment

### ✅ What's Implemented

#### Core Infrastructure
- **LSP Backend** (`src/lsp.rs`): Full LSP protocol support with:
  - Document open/change/close lifecycle
  - Debounce mechanism (250ms default) for performance
  - Single-flight pattern (no stale diagnostics)
  - External command integration for diagnostics

- **Proof Session** (`src/proof_session.rs`):
  - ComradeWorkspace integration for canonical compilation
  - WorkspaceReport consumption
  - Goals panel support with stable anchors
  - ProofSnapshot history (up to 10 snapshots)
  - Loogle indexer integration

- **Document Model** (`src/document.rs`):
  - Parsing and AST extraction
  - Goal extraction from parsed syntax
  - Binding detection
  - Span conversion utilities

#### Advanced Features
- **Tactics Framework** (`src/tactics/`):
  - Registry-based tactic system
  - Edit operations for refactoring
  - Goal-directed tactics
  - Quickfix generation
  - Rewrite tactic support

- **Explanation System** (`src/explain/`):
  - Goal explanation algorithms
  - Blocked goal analysis
  - Inconsistent goal analysis
  - Trace builder for proof reconstruction

- **Refutation Engine** (`src/refute/`):
  - Multiple probes (finite category, rewrite)
  - Witness generation
  - Orchestrator for probe coordination
  - LSP handler for refute commands

- **Diff Engine** (`src/diff/`):
  - Document diff tracking
  - Change-aware diagnostics

- **Search/Indexing** (`src/loogle/`):
  - Workspace-level code search
  - Applicability checking
  - Code action generation

- **Syntax Highlighting** (`src/highlight.rs`):
  - Semantic token stream support
  - Syntax highlighting for diagnostics

#### DB-7 Integration
- **Rename Impact Preview**:
  - Hover shows blast radius (files/scopes affected)
  - Shows predicted cost (typechecks/validations)
  - Proof preservation rate
  - Warning display
  - Two levels of detail (compact/detailed)

### ⚠️ What's Partially Integrated

#### SniperDB Usage
**Currently used:**
- `plan` module for DB-7 rename impact analysis

**Available but unused:**
- `snapshot` - Module snapshots for faster reloading
- `module` - Module metadata and cross-file analysis
- `ops` - Operations API for general DB queries
- `artifact` - Artifact tracking and caching
- `blob_store` - Proof/artifact persistence
- `edit` - Edit tracking and scheduling
- `flight_recorder` - Telemetry and observability
- `store` - Persistent storage backends

### ❌ Known Gaps

#### 1. Limited Cross-File Analysis
- No module-level caching between edits
- Workspace indexing only on demand
- Missing module snapshots for fast reload

#### 2. No Semantic Caching
- Each keystroke recompiles entire modules
- No query-level memoization from SniperDB
- Missing incremental scope invalidation

#### 3. Incomplete Proof State Integration
- Proof snapshots created but not leveraged for navigation
- No proof history UI
- Missing proof artifact linking

#### 4. No Observability
- No telemetry from compilation
- Missing performance metrics
- No cost tracking

#### 5. Limited Quick Fixes
- External commands don't feed into LSP fixes
- No tactic-based quick fixes
- Missing rule suggestions

---

## Recommended Improvement Path

### Phase 1: Semantic Caching Layer (2-3 days, ~20 hours)

**Goal:** Use SniperDB's caching to avoid recompiling unchanged modules.

#### 1.1: Module-Level Snapshots (4-5 hours)
**File:** `src/lsp.rs` + new `src/caching.rs`

- Create `ModuleSnapshot` type with:
  - Module identifier
  - Content hash
  - Compiled form
  - Type information
  - Export environment

- Implement snapshot caching keyed by `(file_id, content_hash)`
- On document change:
  - Check if content hash matches cached snapshot
  - If yes, skip recompilation, reuse snapshot
  - If no, recompile and update snapshot

**Success Criteria:**
- Repeated edits to same content reuse cached module
- No recompilation on undo/redo cycles
- Accurate snapshot invalidation on real changes

#### 1.2: Cross-File Module Metadata (4-5 hours)
**File:** Extend `src/proof_session.rs`

- Create `WorkspaceModuleIndex`:
  - Map: `ModuleId` → `ModuleMetadata`
  - Metadata includes: exports, imports, cost, proof count

- Use SniperDB's `module` API to fetch metadata
- Update index on document changes
- Provide index to:
  - Hover (show import usage)
  - Code actions (suggest imports)
  - Loogle (scoped search)

**Success Criteria:**
- Module metadata available for workspace
- Cross-module references detected
- Import suggestions work

#### 1.3: Incremental Query Memoization (4-5 hours)
**File:** `src/proof_session.rs` + SniperDB `memo` table

- Integrate SniperDB's `MemoTable` for proof queries
- Key queries to cache:
  - Type of goal metavariable
  - Proof state computation
  - Elaboration constraints
  - Unification results

- On proof state change:
  - Check memo table for cached results
  - Invalidate only affected memos (via file_id_fp_map)
  - Reuse cached proofs when dependencies unchanged

**Success Criteria:**
- Memo hits on stable proof states
- No recomputation on unrelated edits
- Performance improvement visible in metrics

#### 1.4: Query Statistics Dashboard (2-3 hours)
**File:** `src/lsp.rs` code action handler

- Expose SniperDB's `stats` via LSP command:
  - Cache hit/miss rates
  - Compilation times per file
  - Total memos in table
  - Memory usage

- Add code action: `"Debug: Show Query Statistics"`
- Display in hover or custom message

**Success Criteria:**
- Stats accessible via LSP command
- Can verify caching is working
- Metrics suitable for performance optimization

**Phase 1 Deliverable:** Module-level caching reduces recompilation by 30-50% on typical edits

---

### Phase 2: Proof Artifact Integration (2-3 days, ~20 hours)

**Goal:** Connect proof preservation tracking to LSP features.

#### 2.1: Artifact Tracking in Goals Panel (4 hours)
**File:** `src/goals_panel.rs` + proof session

- Fetch proof artifacts for each goal:
  - Certified rule ID
  - Artifact blob references
  - Preservation status

- Display in goals panel:
  - Artifact counts
  - Preservation warnings
  - Quick-link to artifact view

**Success Criteria:**
- Goals panel shows artifact status
- Proof preservation visible per-goal
- Warnings trigger on preservation loss

#### 2.2: Proof History Navigation (5 hours)
**File:** Extend `src/proof_session.rs` proof history

- Enhance ProofSnapshot to include:
  - Artifact snapshots at that moment
  - Total proofs preserved/lost
  - Cost delta since last snapshot

- Add LSP commands:
  - `"edgelord.proof-history:next"` - Step forward in history
  - `"edgelord.proof-history:prev"` - Step backward
  - `"edgelord.proof-history:view"` - Show history timeline

- Code action to jump to proof at any point

**Success Criteria:**
- Can navigate proof history
- Each history point shows artifact state
- Visual timeline in diagnostics area

#### 2.3: Proof Artifact Quick Links (4 hours)
**File:** Extend `src/loogle/code_actions.rs`

- When hovering on proof reference:
  - Show artifact details
  - Link to blob store
  - Show which rules depend on this artifact

- Add code action: "Open Artifact Details"
- Display artifact chain in breadcrumb

**Success Criteria:**
- Proof artifacts linkable from editor
- Can trace artifact dependencies
- Visual proof graph (markdown view)

#### 2.4: Proof Preservation Warnings (3-4 hours)
**File:** `src/lsp.rs` diagnostic generation

- Check SniperDB blob store on each compilation:
  - Count proofs before/after
  - Detect preservation loss
  - Flag goals with lost artifacts

- Generate diagnostics:
  - Severity: `Warning` for significant loss (>10%)
  - Code: `proof-preservation-loss`
  - Quick fix: suggest preservation strategies

**Success Criteria:**
- Warnings appear when proof loss detected
- Clear message explains impact
- Suggestions help preserve proofs

**Phase 2 Deliverable:** Proof preservation visible and trackable in editor

---

### Phase 3: Enhanced Hover & Code Actions (2-3 days, ~20 hours)

**Goal:** Leverage SniperDB features for richer LSP features.

#### 3.1: Smart Hover with Module Info (4 hours)
**File:** `src/lsp.rs` hover handler

- On hover over symbol:
  1. Show type (existing)
  2. Add module origin (from module metadata)
  3. Add usage count (from workspace index)
  4. Show import path
  5. Link to definition

**Success Criteria:**
- Hover shows complete symbol context
- Can click to navigate to definition
- Import path visible

#### 3.2: Tactic-Aware Code Actions (6 hours)
**File:** Extend `src/tactics/` + code actions

- Query available tactics for goal:
  - From tactic registry
  - Filtered by applicability
  - Ranked by success probability

- Show as code actions:
  - Top 3 tactics only (avoid menu bloat)
  - Label includes success rate estimate
  - Preview on hover

- DB-7: Show refactoring tactics
  - Impact estimation
  - Cost preview

**Success Criteria:**
- Code actions filtered by applicability
- Top tactics suggested first
- Cost estimates shown

#### 3.3: Module-Aware Loogle (4 hours)
**File:** Extend `src/loogle/`

- Scope search to:
  - Current module (default)
  - Imported modules (option)
  - Entire workspace (option)

- Distinguish results by scope:
  - Local (current file)
  - Imported (available imports)
  - External (requires import)

**Success Criteria:**
- Search respects module boundaries
- Import suggestions available
- Can add import from search result

#### 3.4: Diagnostic Inlay Hints (3-4 hours)
**File:** `src/lsp.rs` inlay hints handler

- Show inline:
  - Goal metavariable targets (types)
  - Proof status at each goal
  - Cost estimates for operations
  - Blast radius previews

**Success Criteria:**
- Inlay hints show rich context
- No performance impact
- Optional per-user preference

#### 3.5: External Command Integration Improvement (2-3 hours)
**File:** `src/lsp.rs` external command handler

- Enhance external command handling:
  - Parse JSON diagnostic output (v1)
  - Feed diagnostics into tactic suggestions
  - Generate quick fixes from external output

- New config:
  - `externalCommandDiagnosticFormat`: "text" | "json"
  - `externalCommandQuickFixFormat`: "none" | "simple" | "full"

**Success Criteria:**
- External commands generate LSP-compatible diagnostics
- Quick fixes from external tools work
- Config controls output format

**Phase 3 Deliverable:** Richer, more contextual LSP features across hovering, code actions, and search

---

### Phase 4: Observability & Metrics (1-2 days, ~10 hours)

**Goal:** Use SniperDB flight recorder for performance tracking.

#### 4.1: Integration with Flight Recorder (4 hours)
**File:** New `src/observability.rs` + `src/lsp.rs`

- Hook into flight recorder during compilation:
  - Record document change events
  - Record compilation time per file
  - Track cache hits/misses
  - Record goal elaboration time

- Aggregate metrics:
  - Total compilation time
  - Average time per file
  - Cache efficiency
  - Goal complexity stats

**Success Criteria:**
- Flight recorder captures LSP operations
- Metrics retrievable via LSP command
- Performance visible to user

#### 4.2: Performance Warning System (3 hours)
**File:** `src/lsp.rs` + observability

- Monitor:
  - Compilation exceeds 1s per file → warning
  - Cache hit rate below 50% → hint
  - Memory usage exceeds threshold → warning

- Show in:
  - Window status message
  - Diagnostic hint (debug info)
  - Custom notification on slow operation

**Success Criteria:**
- Slow operations trigger warnings
- User aware of performance issues
- Can take action (e.g., close unused files)

#### 4.3: Build Time Dashboard (2-3 hours)
**File:** LSP command handler + custom UI

- Command: `"edgelord.debug:show-performance-dashboard"`
- Displays:
  - Compilation timeline (last 10 operations)
  - Cache hit rates per module
  - Memory usage graph
  - Top 5 slowest files
  - Suggestions for improvement

**Success Criteria:**
- Dashboard shows actual performance data
- Actionable insights provided
- Can enable/disable tracking

**Phase 4 Deliverable:** Performance visibility and diagnostics integrated into LSP

---

## Implementation Priority

### Must Do First (Phase 1)
1. **Module snapshots** (4-5h) - Foundation for other optimizations
2. **Incremental memos** (4-5h) - Biggest performance win
3. **Query stats** (2-3h) - Verify it's working

### Should Do Next (Phase 2)
4. **Proof artifact tracking** (4h) - Direct SniperDB integration
5. **Proof history navigation** (5h) - Enhanced UX

### Nice to Have (Phase 3-4)
6. Everything else in Phase 3-4

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| **SniperDB API changes** | Breaks integration | Keep abstraction layer in `caching.rs` |
| **Cache invalidation bugs** | Stale data shown | Add regression tests for cache correctness |
| **Performance regression** | Slower editor | Profile before/after, add benchmarks |
| **Memory bloat** | OOM on large files | Implement snapshot limits, memory bounds |

---

## Success Metrics

### Phase 1 Goals
- [ ] Module snapshots eliminate 30% of recompilations
- [ ] Cache hit rate > 50% on typical edits
- [ ] Stats command works and shows accurate data

### Phase 2 Goals
- [ ] Proof artifacts visible in goals panel
- [ ] Proof history navigation functional
- [ ] Preservation loss warnings trigger correctly

### Phase 3 Goals
- [ ] Smart hover shows complete context
- [ ] Code actions filtered by applicability
- [ ] Loogle respects module boundaries

### Phase 4 Goals
- [ ] Flight recorder captures all operations
- [ ] Performance warnings trigger at thresholds
- [ ] Dashboard shows actionable insights

---

## Architecture Notes

### Key Principles
1. **Separation of Concerns:**
   - `lsp.rs` = LSP protocol only
   - `proof_session.rs` = Proof state management
   - `caching.rs` (new) = SniperDB integration
   - `observability.rs` (new) = Metrics/telemetry

2. **Fail-Closed Design:**
   - Missing cache → recompile (safe)
   - Missing metric → default value (safe)
   - Stale snapshot → invalidate and recompute (safe)

3. **Determinism:**
   - All snapshot keys include content hash
   - Invalidation always conservative
   - Cache keys immutable once stored

### File Changes Summary
- **New Files:** `src/caching.rs`, `src/observability.rs`
- **Modified Files:** `src/lsp.rs`, `src/proof_session.rs`, `src/goals_panel.rs`, `src/loogle/code_actions.rs`
- **No Breaking Changes:** All existing APIs unchanged

---

## Testing Strategy

### Unit Tests
- Snapshot cache key computation
- Snapshot invalidation logic
- Memo table integration
- Flight recorder event recording

### Integration Tests
- End-to-end document edit with caching
- Cache invalidation on real changes
- Performance regression detection
- Proof preservation tracking

### Manual Tests
- Edit a file rapidly → verify cache hits
- Undo/redo → verify snapshot reuse
- Check performance dashboard
- Monitor proof preservation across edits

---

## References

- **SniperDB Module Docs:** `clean_kernel/crates/sniper_db/src/`
- **Existing DB-7 Integration:** `src/lsp.rs` lines 111-170
- **Proof Session:** `src/proof_session.rs` lines 56-220
- **Flight Recorder:** `clean_kernel/crates/sniper_db/src/flight_recorder.rs`

---

**Status:** Ready for Phase 1 implementation
**Owner:** EdgeLorD team
**Next Review:** After Phase 1 completion
