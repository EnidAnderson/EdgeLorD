# Implementation Checklist — EdgeLorD Typed Proof-State System

**Created:** 2026-02-05
**Status:** Ready for implementation
**Based on:** kernel_proof_state_elaboration_plan_v2.md, DIAGNOSTICS_AND_UX_PLAN.md, HOLE_OCCURRENCE_IMPLEMENTATION_PLAN.md, implementation_guide_plan_ordering.md

---

## Planning Documents Summary

All planning documents have been created by Opus 4.6 in the past few hours:

1. **kernel_proof_state_elaboration_plan_v2.md** (Feb 5 15:58) — AUTHORITATIVE
   - Scope: `new_surface_syntax` + `tcb_core` (kernel only)
   - Delivers: Typed metavariables, constraints, unification, ProofState API
   - Supersedes: KERNEL_PROOF_STATE_PLAN.md (v1)

2. **DIAGNOSTICS_AND_UX_PLAN.md** (Feb 5 15:20) — v1
   - Scope: Kernel + EdgeLorD + LSP + lint UX
   - Delivers: StructuredDiagnostic, stable codes, pretty-printing, lint framework
   - Depends on: kernel_proof_state_elaboration_plan_v2.md

3. **HOLE_OCCURRENCE_IMPLEMENTATION_PLAN.md** (Feb 5 14:59) — v3 (FINAL)
   - Scope: Kernel + EdgeLorD hole tracking
   - Delivers: HoleOccurrence with (owner, ordinal) identity, not span-based
   - Status: COMPLETED (per plan v3 notes)

4. **implementation_guide_plan_ordering.md** (Feb 5 16:06) — META guide
   - Explains dependency graph and execution order
   - Defines checkpoints and "done" criteria
   - This document you're reading now follows its structure

5. **KERNEL_PROOF_STATE_PLAN.md** (Feb 5 15:37) — v1 (SUPERSEDED)
   - Historical artifact; v2 is authoritative

---

## Implementation Plan: Dependency-Ordered Tasks

### ✓ COMPLETED: Hole Occurrence Implementation (v3)

Per HOLE_OCCURRENCE_IMPLEMENTATION_PLAN.md v3, this is marked as FINAL and complete.

**Deliverables:**
- `HoleOwner` enum with Def/Rule/TopLevel variants
- `HoleOccurrence` struct with (owner, ordinal) identity
- `CoreBundleV0.holes: Vec<HoleOccurrence>`
- `WorkspaceReport.holes: Vec<HoleOccurrence>`
- EdgeLorD consumes kernel holes instead of syntactic detection

**Status:** Already implemented (plan status: "v3 final")

---

### Phase 0: Lock the Single Source of Truth

**Task #4** — per implementation_guide §3 Phase 0

**Goal:** Establish that kernel ProofState is the authoritative source for goals/types/explain.

**Action Items:**
- [ ] Add `ProofState` stub type to `new_surface_syntax/src/proof_state.rs`
- [ ] Add `pub proof_state: Option<ProofState>` to `CoreBundleV0` in `src/core.rs`
- [ ] Add `pub proof_state: Option<ProofState>` to `WorkspaceReport` in `src/comrade_workspace.rs`
- [ ] Update `Default` impls to set `proof_state: None`

**Success Criteria:**
- Code compiles with optional ProofState fields
- No behavioral change (fields are always None initially)

**Estimated Effort:** 1-2 hours

---

### Gate 0: Data Model + Meta IR Compiles

**Task #5** — per kernel_proof_state_elaboration_plan_v2.md §2, §7 Gate 0

**Goal:** All proof-state data structures compile, no behavioral changes.

**New File:**
- `new_surface_syntax/src/proof_state.rs` (~500 lines)

**Types to Implement:**

```rust
// Meta IDs (INV-1: distinct namespaces)
pub struct ObjMetaId(u32);
pub struct MorMetaId(u32);

// Internal meta-term IR (INV-2: not encoded as HoleId)
pub enum ObjExpr {
    Known(ObjectId),
    Meta(ObjMetaId),
}

pub enum MorExpr {
    Gen(GeneratorId),
    Ref(String),
    Compose(Vec<MorExpr>),
    App { op: GeneratorId, args: Vec<MorExpr> },
    InDoctrine { doctrine: DoctrineKey, term: Box<MorExpr> },
    Meta(MorMetaId),
}

// Types and context
pub struct MorType {
    pub src: ObjExpr,
    pub dst: ObjExpr,
}

pub struct CtxEntry {
    pub name: String,
    pub ty: Option<MorType>,
    pub def: Option<MorExpr>,
    pub span: Option<Span>,
}

pub struct LocalContext {
    pub entries: Vec<CtxEntry>,
    pub doctrine: Option<DoctrineKey>,
}

// Constraints
pub struct ConstraintId(u32);
pub enum ConstraintKind {
    ObjEq { lhs: ObjExpr, rhs: ObjExpr },
    MorEq { lhs: MorExpr, rhs: MorExpr },
    HasType { meta: MorMetaId, expected: MorType },
}
pub struct Constraint {
    pub id: ConstraintId,
    pub kind: ConstraintKind,
    pub provenance: ConstraintProvenance,
}

// Substitution
pub struct MetaSubst {
    obj_map: BTreeMap<ObjMetaId, ObjExpr>,
    mor_map: BTreeMap<MorMetaId, MorExpr>,
}

// Trace
pub struct TraceNodeId(u32);
pub enum TraceNodeKind { /* ... */ }
pub struct TraceNode { /* ... */ }
pub struct TraceEdge { /* ... */ }
pub struct ElaborationTrace {
    pub nodes: Vec<TraceNode>,
    pub edges: Vec<TraceEdge>,
}

// ProofState API
pub enum GoalStatus {
    Unsolved,
    Solved(MorExpr),
    Blocked { depends_on: Vec<MorMetaId> },
    Inconsistent { conflicts: Vec<ConstraintId> },
}

pub struct GoalState {
    pub id: MorMetaId,
    pub name: String,
    pub owner: HoleOwner,  // from HOLE_OCCURRENCE_IMPLEMENTATION_PLAN v3
    pub span: Option<Span>,
    pub local_context: LocalContext,
    pub expected_type: MorType,
    pub status: GoalStatus,
}

pub struct ProofState {
    pub goals: Vec<GoalState>,
    pub constraints: Vec<Constraint>,
    pub subst: MetaSubst,
    pub trace: ElaborationTrace,
}
```

**Tests to Add:** (12 total)
- 8 MetaId unit tests (allocation, ordering, determinism)
- 4 MetaSubst property tests (idempotence, occurs-check, composition, determinism)

**Files to Modify:**
- `new_surface_syntax/src/lib.rs` — add `pub mod proof_state;`
- `new_surface_syntax/Cargo.toml` — verify no new deps needed

**Success Criteria:**
- `cargo check` passes
- All 12 tests pass
- No behavioral change to existing compilation path

**Estimated Effort:** 4-6 hours

---

### Gate 1: Bidirectional Elaboration Skeleton (No Solving)

**Task #6** — per kernel_proof_state_elaboration_plan_v2.md §4, §7 Gate 1

**Goal:** Introduce typed elaborator that allocates metas and generates constraints.

**New Struct:**

```rust
pub struct TypedElaborator {
    scope_stack: Vec<TypedScope>,
    obj_meta_counter: u32,
    mor_meta_counter: u32,
    constraint_counter: u32,
    trace_node_counter: u32,
    metas: Vec<MetaInfo>,
    constraints: Vec<Constraint>,
    subst: MetaSubst,
    trace: ElaborationTrace,
    current_owner: HoleOwner,
    current_ordinal: u32,
}

struct TypedScope {
    entries: BTreeMap<String, CtxEntry>,
}
```

**Elaboration Algorithm:**

```rust
impl TypedElaborator {
    // Bidirectional elaboration entry points
    fn check_morphism(&mut self, expr: &SExpr, expected: &MorType)
        -> Result<MorExpr, ElaborationError>;

    fn infer_morphism(&mut self, expr: &SExpr)
        -> Result<(MorExpr, MorType), ElaborationError>;

    // Meta allocation
    fn fresh_obj_meta(&mut self) -> ObjMetaId;
    fn fresh_mor_meta(&mut self, name: &str, ctx: LocalContext, expected: MorType)
        -> MorMetaId;

    // Constraint generation
    fn emit_constraint(&mut self, kind: ConstraintKind, provenance: ConstraintProvenance);

    // Context management
    fn snapshot_context(&self) -> LocalContext;
}
```

**Key Rules:**
- `?name` symbols → allocate `MorMetaId`, emit `TraceNode::MetaIntroduced`
- Bare symbols → lookup in scope, NOT metavariables (rigid)
- Composition `(f g h)` → infer each component, emit boundary constraints
- `(def x term)` → set `current_owner = Def(x)`, reset ordinal

**Tests to Add:** (10 total)
- 8 "goal target exists" tests
- 2 trace recording tests

**Files to Modify:**
- `new_surface_syntax/src/elaborate.rs` — add `TypedElaborator` alongside `Elaborator`
- `new_surface_syntax/src/lib.rs` — add `typed_elaborate_query()`

**Success Criteria:**
- Can compile file with `?name` holes
- ProofState has populated `goals` with `Unsolved` status
- Constraints are generated but not solved yet
- Trace records meta introductions

**Estimated Effort:** 8-12 hours

---

### Gate 2: Object Unification Solver (Endpoints Only)

**Task #7** — per kernel_proof_state_elaboration_plan_v2.md §5, §7 Gate 2

**Goal:** Solve object metavariables to infer composition endpoints.

**New File:**
- `new_surface_syntax/src/solver.rs`

**Solver Algorithm:**

```rust
pub fn solve_constraints(
    constraints: &mut Vec<Constraint>,
    subst: &mut MetaSubst,
    trace: &mut ElaborationTrace,
) -> Result<(), SolverError> {
    let mut worklist = VecDeque::from_iter(0..constraints.len());
    let fuel = constraints.len() * 2;

    for _ in 0..fuel {
        if worklist.is_empty() { break; }
        let cid = worklist.pop_front().unwrap();

        match &constraints[cid].kind {
            ObjEq { lhs, rhs } => {
                match (apply_obj(lhs, subst), apply_obj(rhs, subst)) {
                    (Known(a), Known(b)) if a == b => {
                        // Satisfied
                    }
                    (Known(a), Known(b)) => {
                        return Err(Conflict { lhs: a, rhs: b });
                    }
                    (Meta(m), Known(obj)) | (Known(obj), Meta(m)) => {
                        // Occurs check
                        if occurs_in_obj(m, &Known(obj)) {
                            return Err(OccursCheck);
                        }
                        subst.obj_map.insert(m, Known(obj));
                        // Re-enqueue dependent constraints
                    }
                    (Meta(m1), Meta(m2)) => {
                        // Tie-break: bind larger to smaller
                        let (hi, lo) = if m1 > m2 { (m1, m2) } else { (m2, m1) };
                        subst.obj_map.insert(hi, Meta(lo));
                    }
                }
            }
            _ => { /* morphism constraints handled in Gate 3 */ }
        }
    }

    Ok(())
}
```

**Tests to Add:** (10 total)
- Object equality constraints (same/different/meta)
- Tie-breaking for two metas
- Occurs-check detection
- Composition boundary inference

**Success Criteria:**
- Composition chains infer endpoints through `ObjEq` constraints
- `cargo test` passes all unification tests

**Estimated Effort:** 6-8 hours

---

### Gate 3: Morphism-Term Unification Solver

**Task #8** — per kernel_proof_state_elaboration_plan_v2.md §5, §7 Gate 3

**Goal:** Solve morphism metavariables to concrete terms.

**Extend `solver.rs`:**

```rust
// Handle MorEq constraints
MorEq { lhs, rhs } => {
    match (apply_mor(lhs, subst), apply_mor(rhs, subst)) {
        (Meta(m), term) | (term, Meta(m)) if !is_meta(&term) => {
            if occurs_in_mor(m, &term) {
                return Err(OccursCheck);
            }
            subst.mor_map.insert(m, term);
            // Emit trace node
        }
        (Compose(cs1), Compose(cs2)) if cs1.len() == cs2.len() => {
            // Structural unification
            for (c1, c2) in cs1.iter().zip(cs2.iter()) {
                emit_constraint(MorEq { lhs: c1.clone(), rhs: c2.clone() });
            }
        }
        (Gen(g1), Gen(g2)) if g1 == g2 => { /* satisfied */ }
        (App { op: o1, args: a1 }, App { op: o2, args: a2 })
            if o1 == o2 && a1.len() == a2.len() => {
            // Unify args pairwise
        }
        _ => {
            return Err(Mismatch);
        }
    }
}
```

**Tests to Add:** (10 total)
- 6 constraint generation tests
- 4 snapshot tests for solving

**Success Criteria:**
- Morphism metas can be solved to concrete `Compose`/`Gen`/`App` terms
- Structural unification works for matching constructors

**Estimated Effort:** 8-10 hours

---

### Gate 4: Dependency Analysis + Blocked Classification

**Task #9** — per kernel_proof_state_elaboration_plan_v2.md §6, §7 Gate 4

**Goal:** Classify goal status with dependency tracking.

**Algorithm:**

```rust
pub fn classify_goals(
    goals: &mut [GoalState],
    constraints: &[Constraint],
    subst: &MetaSubst,
) {
    // Build mention graph
    let mentions: BTreeMap<MorMetaId, BTreeSet<ConstraintId>> = build_mentions(constraints);

    for goal in goals {
        if subst.mor_map.contains_key(&goal.id) {
            goal.status = Solved(subst.mor_map[&goal.id].clone());
        } else {
            // Check if blocked
            let deps = find_dependencies(goal.id, constraints, subst);
            if !deps.is_empty() {
                goal.status = Blocked { depends_on: deps };
            } else if has_conflicts(goal.id, constraints) {
                let conflicts = find_conflicts(goal.id, constraints);
                goal.status = Inconsistent { conflicts };
            } else {
                goal.status = Unsolved;
            }
        }
    }
}
```

**Tests to Add:** (6 total)
- 4 trace tests (blocked dependencies, conflict chains)
- 2 snapshot tests

**Success Criteria:**
- Blocked goals report specific meta dependencies
- Inconsistent goals report conflicting constraints

**Estimated Effort:** 6-8 hours

---

### Gate 5: Trace DAG + Explanation Queries

**Task #10** — per kernel_proof_state_elaboration_plan_v2.md §6, §7 Gate 5

**Goal:** Implement structured explanation queries.

**API:**

```rust
impl ProofState {
    pub fn explain_goal(&self, meta: MorMetaId) -> ExplanationGraph {
        // Extract subgraph: meta introduction → constraints → solve attempts
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Find intro node
        for node in &self.trace.nodes {
            if let MetaIntroduced { metavar, .. } = node.kind {
                if metavar == meta {
                    nodes.push(node.clone());
                }
            }
        }

        // Find related constraints
        for constraint in &self.constraints {
            if mentions_meta(constraint, meta) {
                nodes.push(TraceNode::ConstraintGenerated { ... });
            }
        }

        // Find solve steps
        for node in &self.trace.nodes {
            if let UnificationStep { metavar, .. } = node.kind {
                if metavar == meta {
                    nodes.push(node.clone());
                }
            }
        }

        ExplanationGraph { nodes, edges }
    }

    pub fn explain_conflict(&self, constraints: &[ConstraintId]) -> String {
        // Trace back to incompatible derivations
    }
}
```

**Tests to Add:** (4 total)
- Snapshot tests for explanation rendering

**Success Criteria:**
- Can query "why is ?x blocked?" and get causal graph
- Conflict explanations identify incompatible constraints

**Estimated Effort:** 6-8 hours

---

### Gate 6: Zonking + Lowering to tcb_core::MorphismTerm

**Task #11** — per kernel_proof_state_elaboration_plan_v2.md §7 Gate 6

**Goal:** Apply substitution and lower to legacy format.

**Zonking:**

```rust
pub fn zonk_mor_expr(expr: &MorExpr, subst: &MetaSubst) -> MorExpr {
    match expr {
        Meta(m) => {
            if let Some(solution) = subst.mor_map.get(m) {
                zonk_mor_expr(solution, subst)  // recursive
            } else {
                Meta(*m)  // unsolved, keep as meta
            }
        }
        Compose(cs) => Compose(cs.iter().map(|c| zonk_mor_expr(c, subst)).collect()),
        App { op, args } => App {
            op: *op,
            args: args.iter().map(|a| zonk_mor_expr(a, subst)).collect(),
        },
        _ => expr.clone(),
    }
}
```

**Lowering:**

```rust
pub fn lower_to_morphism_term(expr: &MorExpr, subst: &MetaSubst) -> MorphismTerm {
    let zonked = zonk_mor_expr(expr, subst);
    match zonked {
        Gen(g) => MorphismTerm::Generator { id: g, ... },
        Compose(cs) => MorphismTerm::Compose {
            components: cs.iter().map(|c| lower_to_morphism_term(c, subst)).collect(),
            ...
        },
        Meta(m) => MorphismTerm::Hole(m.as_u32()),  // unsolved → hole
        ...
    }
}
```

**Tests to Add:** (4 total)
- Regression tests for lowering

**Success Criteria:**
- Zonking produces fully-substituted `MorExpr`
- Lowering produces valid `MorphismTerm` bundle

**Estimated Effort:** 4-6 hours

---

### Gate 7: Make ProofState Authoritative Output

**Task #12** — per kernel_proof_state_elaboration_plan_v2.md §7 Gate 7

**Goal:** Switch to typed elaborator by default, return ProofState in all paths.

**Changes:**

```rust
// In src/lib.rs
pub fn compile_comrade_db(
    db: &mut ComradeQueryDb,
    file_id: FileId,
    options_id: OptionsId,
) -> Result<(CoreBundleV0, ProofState), SurfaceError> {
    // Parse and expand as before
    let expanded = expand_query(db, module, options_id)?;

    // Use TypedElaborator by default
    let mut elaborator = TypedElaborator::new();
    let (forms, proof_state) = elaborator.elaborate(&expanded.core_forms)?;

    // Package into bundle
    let bundle = CoreBundleV0 {
        forms,
        proof_state: Some(proof_state.clone()),
        ...
    };

    Ok((bundle, proof_state))
}
```

```rust
// In comrade_workspace.rs
pub fn did_open(&mut self, key: DocumentKey, text: String)
    -> Result<WorkspaceReport, SurfaceError> {
    let file_id = self.open_document(key.clone(), text);
    let (bundle, proof_state) = compile_comrade_db(&mut self.service.db, file_id, ...)?;

    Ok(WorkspaceReport {
        diagnostics: /* ... */,
        fingerprint: /* ... */,
        revision: /* ... */,
        bundle: Some(bundle),
        proof_state: Some(proof_state),
    })
}
```

**Tests to Add:** (2 total)
- Integration test: WorkspaceReport has ProofState
- Integration test: EdgeLorD reads goal from ProofState

**Success Criteria:**
- Open file with `?x` holes → WorkspaceReport has typed goals
- Goals show stable MorMetaId, expected type, context
- Blocked/Unsolved/Inconsistent status with explanation

**Estimated Effort:** 4-6 hours

---

## Phase 1 Total Estimated Effort: 54-76 hours

**Checkpoint:** After Gate 7, the kernel proof-state system is complete and authoritative.

---

### Phase 2: Structured Diagnostics as Kernel Output Format

**Task #13** — per DIAGNOSTICS_AND_UX_PLAN.md, implementation_guide §3 Phase 2

**Prerequisites:** All of Phase 1 (Gates 0-7) complete.

**Subtasks:**

1. **Introduce StructuredDiagnostic (4-6 hours)**
   - Create `new_surface_syntax/src/diagnostics.rs`
   - Define `StructuredDiagnostic` with code registry
   - Define `DiagnosticCode` enum (ML-P-001, ML-E-003, etc.)
   - Implement `to_diagnostic()` trait for all error types

2. **Replace stringly errors (6-8 hours)**
   - Update `ParseError`, `MacroError`, `ElaborationError` to produce `StructuredDiagnostic`
   - Add `LabeledSpan` with optional spans (INV-4: no fake spans)
   - Thread `DiagnosticCode` through all error constructors

3. **Add printers for proof-state terms (4-6 hours)**
   - Implement `Display` for `ObjExpr`, `MorExpr`, `MorType`
   - Implement `Display` for `GoalState`, `Constraint`, `TraceNode`
   - Use Wadler-Lindig `Doc` pretty-printing model
   - Avoid `Debug` dumps in user-facing messages

4. **Enforce "no fake spans" (2-3 hours)**
   - Add CI grep: `grep -r "Span::new(0" new_surface_syntax/ && exit 1`
   - Add tests verifying `span: None` for macro-generated code
   - Update all span construction to use `Option<Span>`

**Tests to Add:** 50+ snapshot tests for diagnostic rendering

**Success Criteria:**
- All diagnostics have stable codes (ML-*)
- Diagnostics reference ProofState (e.g., "unsolved goal ?0" with target type)
- No fake spans in codebase (CI enforced)

**Estimated Effort:** 16-23 hours

---

### Phase 3: Lints (Typed, After Kernel Proof-State Exists)

**Task #14** — per DIAGNOSTICS_AND_UX_PLAN.md, implementation_guide §3 Phase 3

**Prerequisites:** Phase 1 and Phase 2 complete.

**Lint Categories:**

1. **Purely syntactic (4-6 hours):**
   - Unused touch
   - Duplicate touch
   - Redundant begin
   - Empty composition

2. **Goal-aware (6-8 hours):**
   - Suspicious holes (same name in same owner)
   - Repeated hole names (different ordinals)
   - Holes with no constraints

3. **Type-aware (8-10 hours):**
   - Shadowing with type mismatch
   - Rule identity under definitional equality
   - Composition endpoint mismatch (redundant with constraints, but nicer message)

**Framework:**

```rust
pub trait Lint {
    fn name(&self) -> &'static str;
    fn check(&self, bundle: &CoreBundleV0, proof_state: &ProofState) -> Vec<LintDiagnostic>;
}

pub struct LintConfig {
    pub enabled: BTreeSet<String>,
}

pub struct LintDiagnostic {
    pub lint_name: String,
    pub message: String,
    pub span: Option<Span>,
    pub quick_fixes: Vec<QuickFix>,
}
```

**Tests to Add:** 50+ snapshot tests (one per lint + variations)

**Success Criteria:**
- Lints leverage typed context (no "unknown" fallbacks)
- Quick fixes available where applicable

**Estimated Effort:** 18-24 hours

---

### Phase 4: EdgeLorD/LSP Consumption (Last)

**Task #15** — per implementation_guide §3 Phase 4

**Prerequisites:** All kernel work (Phases 1-3) complete.

**EdgeLorD Integration:**

1. **Map StructuredDiagnostic → LSP Diagnostic (3-4 hours)**
   - Update `src/lsp.rs::publish_diagnostics()`
   - Map `DiagnosticCode` to LSP code strings
   - Map `LabeledSpan` to LSP ranges
   - Include explanation availability in `data` field

2. **Hover displays kernel goal state (4-6 hours)**
   - Update `src/lsp.rs::hover()`
   - Read `ProofState` from `ProofSession`
   - Display goal target type, context, status
   - Fallback to syntactic info on parse failure

3. **Inlay hints display kernel-derived types (3-4 hours)**
   - Update `src/lsp.rs::inlay_hint()`
   - Show target types from `GoalState.expected_type`
   - Show solved terms for `GoalStatus::Solved`

4. **Code actions leverage explanations (6-8 hours)**
   - Implement "Explain this error" action
   - Query `proof_state.explain_goal(meta)`
   - Render trace as markdown in LSP message
   - Implement quick fixes from lint diagnostics

**Graceful Degradation:**
- Parse failure → show syntactic info (existing behavior)
- Parse success → show kernel ProofState (new behavior)

**Tests to Add:** Integration tests for LSP protocol

**Success Criteria:**
- Hover on `?x` shows typed goal with context
- Diagnostics include "Explain" code actions when available
- Inlay hints show target types (not "unknown")

**Estimated Effort:** 16-22 hours

---

## Total Estimated Effort

| Phase | Estimated Hours |
|-------|----------------|
| Phase 0 | 1-2 |
| Gate 0 | 4-6 |
| Gate 1 | 8-12 |
| Gate 2 | 6-8 |
| Gate 3 | 8-10 |
| Gate 4 | 6-8 |
| Gate 5 | 6-8 |
| Gate 6 | 4-6 |
| Gate 7 | 4-6 |
| **Phase 1 Total** | **54-76** |
| Phase 2 | 16-23 |
| Phase 3 | 18-24 |
| Phase 4 | 16-22 |
| **Grand Total** | **105-147 hours** |

---

## Implementation Strategy for AI

### One-File Loop (per implementation_guide §4)

For each gate/task:
1. Pick ONE file to modify
2. Get it compiling + tested
3. Move to next compile error
4. Keep changes minimal and reversible

### Checkpoints (per implementation_guide §5)

After each gate:
- [ ] `cargo check` passes
- [ ] Gate-specific test suite passes
- [ ] Snapshot tests produce stable output
- [ ] No fake spans introduced
- [ ] Determinism verified (same input → same output)

### Acceptance Programs (keep in `tests/fixtures/`)

```lisp
;; tests/fixtures/single_hole.maclane
(touch f)
(def f ?h)

;; tests/fixtures/composition_hole.maclane
(touch x)
(def x (compose ?f g))

;; tests/fixtures/rule_hole.maclane
(rule (compose ?f g) (compose g ?f) (meta (provenance "test")))

;; tests/fixtures/inconsistent_boundary.maclane
(touch bad)
(def bad (compose (f : A → B) (g : C → D)))  ;; force B ≠ C conflict
```

### Definition of Done (per implementation_guide §6)

The systemic work is complete when:
- [ ] Kernel proof-state exists and is deterministic
- [ ] Can explain blocked/conflict goals with trace
- [ ] Diagnostics render proof-state-derived messages (no Debug dumps)
- [ ] EdgeLorD shows typed goals from kernel (not syntax)
- [ ] All 150+ tests pass
- [ ] CI enforces no fake spans

---

## References

- **Authoritative plan:** kernel_proof_state_elaboration_plan_v2.md
- **Consumer plan:** DIAGNOSTICS_AND_UX_PLAN.md
- **Completed plan:** HOLE_OCCURRENCE_IMPLEMENTATION_PLAN.md (v3 final)
- **Meta-guide:** implementation_guide_plan_ordering.md

**Next step:** Begin with Task #4 (Phase 0) or Task #5 (Gate 0).
