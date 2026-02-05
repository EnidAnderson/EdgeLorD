# Kernel Proof-State Elaboration Plan

**Status:** v1 (for review)
**Scope:** Kernel only (new_surface_syntax + tcb_core) — no EdgeLorD/LSP/lint planning

---

## 0. Executive Summary

### What changes

The current elaboration pipeline (`parse → expand → elaborate`) produces **untyped** `MorphismTerm` values with `Hole(u32)` placeholders. Holes carry no context, no type, and no identity beyond an FNV hash of their name. The `Scope` struct tracks binders and definitions by name only — it has no typing information.

This plan replaces that with a **typed proof-state elaborator** that:

1. Introduces **metavariables** (`MetavarId`) as first-class citizens during elaboration, each with a typed local context and a target type (domain/codomain judgment).
2. **Generates constraints** during elaboration — boundary-matching, composition-compatibility, and arity constraints — recorded with full provenance.
3. **Solves constraints** via first-order unification (reusing the existing `unify_patterns` infrastructure in `tcb_core::pattern`), extended to operate on `ObjectTerm` equalities.
4. Produces a **ProofState** output — goals, constraints, partial substitutions, and an explanation trace — that downstream tools (EdgeLorD, linting, LSP) can consume as an authoritative API.

### Why it's the "systems change"

Every downstream feature — typed goal display, "what does this hole need?", explain-this-error, intelligent code actions — is blocked on the kernel providing authoritative type information for holes. Currently EdgeLorD detects goals syntactically and says "target: unknown." This plan makes the kernel the single source of truth.

### What it unlocks

- **Typed goal states**: "Goal `?f`: morphism `A → B` in context `[x : A, g : B → C]`"
- **Constraint-driven explanations**: "This hole is unsolved because `source(?f)` must equal `A` but no binding provides `A → ?`"
- **Partial substitution display**: "After solving, `?f` = `g ∘ h` (from constraint at line 4)"
- **Incremental solving**: Same constraint set → same solution (deterministic)
- **Foundation for tactics/automation**: Constraints are the input to automated proof search

### Definition of done

The system is "the real elaborator" when:

1. Every `?`-prefixed hole in source produces a `MetavarId` with a typed `GoalState`.
2. Every `GoalState` has a target type (`MorphismType` or `ObjectJudgment`) and a local context with binders and their types.
3. Constraint solving is sound: if reported solved, the substitution satisfies all constraints.
4. Same source → same `MetavarId`s, same constraint ordering, same trace rendering.
5. No fake spans: all provenance is `Option<Span>` with honest `None` for generated terms.
6. The `ProofState` API is consumed by at least one downstream client (EdgeLorD integration test).
7. All 40+ tests pass, including snapshot tests for goal summaries and traces.

---

## 1. Requirements & Invariants

### INV-M1: Metavariable identity

Every `?name` hole occurrence maps to a unique `MetavarId` that is stable within a compilation snapshot. Two holes `?x` at different positions produce different metavariables even if they share the same name. Identity is `(owner: HoleOwner, ordinal: u32)` — not the FNV hash, not the span.

### INV-M2: Typed context

Every metavariable has:
- A **target type**: `MorphismJudgment { expected_source: Option<ObjectId>, expected_target: Option<ObjectId> }` — what the hole must inhabit.
- A **local context**: `Vec<ContextEntry>` — binders in scope with their types.
- A **constraint set**: `Vec<ConstraintId>` — constraints that mention this metavariable.
- A **provenance**: `MetavarProvenance { span: Option<Span>, owner: HoleOwner, name: String }`.

### INV-M3: No fake spans

Provenance is recorded honestly. If a span is unavailable (e.g., kernel-generated term, macro expansion without invocation tracking), use `None`. Never use `Span::new(0, 0)`.

### INV-M4: Determinism

Same source text → same `MetavarId` assignment, same constraint ordering, same substitution, same trace rendering. Enforced by:
- Sequential ordinal assignment during deterministic AST walk.
- `BTreeMap`/`BTreeSet` for all collections.
- Deterministic tie-breaking in unification (lower `MetavarId` preferred).

### INV-M5: Soundness boundary

If the solver reports a metavariable as **solved**, the substitution `σ` satisfies:
- `σ(?m)` is a well-typed `MorphismTerm` under the local context.
- All constraints mentioning `?m` are satisfied under `σ`.
- Occurs-check passed (no cyclic substitutions).

If the solver cannot determine solvability, the metavariable is **unsolved** (not falsely reported).

### INV-M6: Goal status semantics

| Status | Meaning |
|--------|---------|
| `Unsolved` | No substitution found; constraints are consistent but incomplete. |
| `Solved(term)` | Unique solution found; substitution satisfies all constraints. |
| `Blocked(reason)` | Solvable in principle but waiting on another metavariable's solution. |
| `Inconsistent(conflicts)` | Constraints are contradictory; no solution exists. |

### INV-M7: Bare symbols

Bare symbols (non-`?`) that currently become `MorphismTerm::Hole(hash)` via `sexpr_to_hyperedge` are **rigid constants** in the new system, not metavariables. They are looked up in the scope as generator references. If lookup fails, an `UnboundSymbol` error is emitted. This is the existing behavior for elaboration; only `sexpr_to_hyperedge` conflates them, and the plan does not change that static helper (it operates outside the typed elaborator).

---

## 2. Core Representations (Data Model)

### 2.1 MetavarId

```rust
/// Unique metavariable identifier within a compilation snapshot.
/// Stable, deterministic, and ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetavarId(u32);

impl MetavarId {
    pub fn as_u32(self) -> u32 { self.0 }
}
```

**Identity rule:** Sequential allocation during deterministic AST walk. The first `?`-hole encountered gets `MetavarId(0)`, the second gets `MetavarId(1)`, etc.

**Not hash-based:** Unlike the current `HoleId = u32` which is an FNV hash (collisions possible), `MetavarId` is a sequential counter (unique by construction).

**Ordering:** Total order by `u32` value. Used for deterministic iteration.

**Serialization:** `u32` only. No serde needed for kernel-internal use; downstream serialization is consumer's responsibility.

### 2.2 Typed Context

```rust
/// A single entry in a metavariable's local context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    /// Binder name (from `touch`).
    pub name: String,
    /// Type of this binder, if known.
    /// `None` means the binder was introduced but never given a type
    /// (e.g., bare `touch x` without a subsequent typed definition).
    pub ty: Option<MorphismType>,
    /// The definition body, if this binder has one (from `def`).
    pub def_body: Option<MorphismTerm>,
    /// Span where this binder was introduced.
    pub span: Option<Span>,
}

/// Snapshot of the elaboration context at a metavariable introduction point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalContext {
    /// Entries in scope order (outermost first).
    pub entries: Vec<ContextEntry>,
    /// The ambient doctrine, if any.
    pub doctrine: Option<DoctrineKey>,
}
```

**Invariants:**
- `entries` is ordered: outermost binder first.
- No duplicate names within a single `LocalContext` (enforced by elaboration's existing duplicate check).
- `ty` is `None` for bare `touch` binders that have no associated definition. Once a `def` provides a term, the elaborator attempts to infer the type via `TypingContext::check_morphism` and fills `ty`.

### 2.3 Type Discipline

Mac Lane's kernel operates in a **simply-typed** setting for morphisms:

> A morphism `f` has type `A → B` where `A` and `B` are objects. The type is `MorphismType { source: ObjectId, target: ObjectId }`.

This is exactly what `TypingContext::check_morphism` already computes. The elaborator extends this to holes:

```rust
/// What a metavariable is expected to inhabit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedType {
    /// Morphism from source to target.
    Morphism {
        source: ObjectConstraint,
        target: ObjectConstraint,
    },
    /// Object (if the hole appears in object position — future extension).
    Object,
}

/// A constraint on a single endpoint (source or target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectConstraint {
    /// Known concrete object.
    Known(ObjectId),
    /// Unknown — represented by a fresh object metavariable.
    Metavar(MetavarId),
}
```

**Type universe:** We do NOT implement dependent types, polymorphism, or universe levels. The type system is: objects are generators/constructors, morphisms have source and target objects. This matches the existing `TypingContext`.

### 2.4 Constraints

```rust
/// Unique constraint identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintId(u32);

/// A constraint generated during elaboration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub id: ConstraintId,
    pub kind: ConstraintKind,
    pub provenance: ConstraintProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    /// Two objects must be equal: `A = B`.
    /// Arises from composition boundary matching.
    ObjectEq {
        lhs: ObjectConstraint,
        rhs: ObjectConstraint,
    },

    /// A metavariable must have a specific morphism type.
    /// Arises from the position a hole appears in.
    HasType {
        metavar: MetavarId,
        expected: ExpectedType,
    },

    /// Two morphism terms must be equal (after substitution).
    /// Arises from rule matching, definition unfolding.
    MorphismEq {
        lhs: MorphismTerm,
        rhs: MorphismTerm,
    },

    /// A metavariable's source must match a specific object.
    SourceEq {
        metavar: MetavarId,
        object: ObjectConstraint,
    },

    /// A metavariable's target must match a specific object.
    TargetEq {
        metavar: MetavarId,
        object: ObjectConstraint,
    },
}

/// Where a constraint came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintProvenance {
    /// Source span (optional — None for generated constraints).
    pub span: Option<Span>,
    /// Human-readable reason.
    pub reason: ConstraintReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintReason {
    /// Composition boundary: target(f) must equal source(g).
    CompositionBoundary { left_index: usize, right_index: usize },
    /// Hole appears in morphism position with known endpoints.
    HoleInContext,
    /// Definition body must match declared binder.
    DefinitionBinding { name: String },
    /// Rule LHS/RHS endpoint consistency.
    RuleBoundary,
    /// Inferred from surrounding context.
    Inferred,
}
```

**Ordering/determinism:** Constraints are stored in a `Vec<Constraint>` in allocation order (sequential `ConstraintId`). This is deterministic because elaboration is a deterministic walk.

### 2.5 Substitution

```rust
/// Partial substitution mapping metavariables to terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaSubst {
    /// Metavariable → MorphismTerm mapping.
    morphism_map: BTreeMap<MetavarId, MorphismTerm>,
    /// Object metavariable → ObjectId mapping.
    object_map: BTreeMap<MetavarId, ObjectId>,
}

impl MetaSubst {
    pub fn new() -> Self { ... }

    /// Apply substitution to a MorphismTerm, replacing Hole(id) with solved terms.
    pub fn apply_morphism(&self, term: &MorphismTerm) -> MorphismTerm { ... }

    /// Apply substitution to an ObjectConstraint.
    pub fn apply_object(&self, obj: &ObjectConstraint) -> ObjectConstraint { ... }

    /// Compose two substitutions: self ∘ other.
    pub fn compose(&self, other: &MetaSubst) -> MetaSubst { ... }

    /// Check if a metavariable is solved.
    pub fn is_solved(&self, m: MetavarId) -> bool { ... }

    /// Get the solution for a metavariable.
    pub fn get_morphism(&self, m: MetavarId) -> Option<&MorphismTerm> { ... }
}
```

**Invariants:**
- Substitution is **idempotent**: `σ(σ(t)) = σ(t)` (enforced by finalization after each unification step, matching `finalize_subst` in `pattern/mod.rs`).
- No cycles (occurs-check enforced).
- Keys are sorted (`BTreeMap`) for deterministic iteration.

### 2.6 Trace Model

```rust
/// A node in the explanation trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
    pub id: TraceNodeId,
    pub kind: TraceNodeKind,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraceNodeId(u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceNodeKind {
    /// "Metavariable ?x introduced here"
    MetavarIntroduced {
        metavar: MetavarId,
        expected_type: ExpectedType,
    },
    /// "Constraint C generated because ..."
    ConstraintGenerated {
        constraint: ConstraintId,
        reason: ConstraintReason,
    },
    /// "Unification step: solved ?x = term"
    UnificationStep {
        metavar: MetavarId,
        solution: MorphismTerm,
    },
    /// "Unification failed: cannot unify A with B"
    UnificationFailure {
        lhs: String, // pretty-printed
        rhs: String,
    },
    /// "Blocked: ?x depends on unsolved ?y"
    Blocked {
        metavar: MetavarId,
        depends_on: MetavarId,
    },
}

/// Edge between trace nodes (causality).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEdge {
    pub from: TraceNodeId,
    pub to: TraceNodeId,
    pub label: String,
}

/// The full trace graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElaborationTrace {
    pub nodes: Vec<TraceNode>,
    pub edges: Vec<TraceEdge>,
}
```

**Determinism:** Nodes and edges are appended in elaboration order. No sorting needed; order is inherently deterministic.

**Attachment:** Each trace node carries an `Option<Span>`. Pretty-printable snapshots are generated on demand, not stored (to avoid bloating the trace).

---

## 3. Elaboration Algorithm (Pipeline)

### 3.1 Overview

```
Source text
  ↓ [existing: parse → expand]
Vec<SExpr>  (expanded forms)
  ↓ [NEW: typed elaboration]
(Vec<CoreForm>, ProofState)
  ↓ [existing: package into CoreBundleV0]
CoreBundleV0 + ProofState
```

The new typed elaborator replaces the current `Elaborator::elaborate()` method. It produces the same `Vec<CoreForm>` output (backward compatible) plus a `ProofState` sidecar.

### 3.2 TypedElaborator struct

```rust
pub struct TypedElaborator {
    /// Lexical scope stack.
    scope_stack: Vec<TypedScope>,
    /// All allocated metavariables.
    metavars: Vec<MetavarInfo>,
    /// All generated constraints.
    constraints: Vec<Constraint>,
    /// Current substitution (updated as constraints are solved).
    subst: MetaSubst,
    /// Explanation trace.
    trace: ElaborationTrace,
    /// Sequential counters.
    next_metavar: u32,
    next_constraint: u32,
    next_trace_node: u32,
    /// Current owner context (for HoleOwner assignment).
    current_owner: HoleOwner,
    /// Ordinal counter within current owner.
    current_ordinal: u32,
    /// Typing context (shared with kernel).
    typing_ctx: TypingContext,
    /// Compiled macros/sugars (forwarded unchanged).
    macros: Vec<SugarForm>,
    /// Compiled rules.
    rules: Vec<CompiledRule>,
}

/// Per-scope typing information.
struct TypedScope {
    /// Binder names and their types.
    binders: BTreeMap<String, ContextEntry>,
}

/// Full information about a metavariable.
pub struct MetavarInfo {
    pub id: MetavarId,
    pub name: String,
    pub owner: HoleOwner,
    pub ordinal: u32,
    pub local_context: LocalContext,
    pub expected_type: ExpectedType,
    pub span: Option<Span>,
    /// Constraint IDs that mention this metavar.
    pub constraints: Vec<ConstraintId>,
}
```

### 3.3 Elaboration algorithm step by step

**Phase 1: Form elaboration** (walks the expanded SExpr list)

```
for each SExpr in expanded_forms:
    match form:
        (touch name) →
            1. Insert `name` into current scope with `ty: None`
            2. Record ContextEntry { name, ty: None, def_body: None, span }

        (def name body) →
            1. Check `name` was touched (existing check)
            2. Check no duplicate (existing check)
            3. Elaborate `body` via typed_sexpr_to_morphism(body)
               → This introduces metavariables for ?-holes
               → This generates constraints from composition structure
            4. Attempt to type the result: typing_ctx.check_morphism(elaborated_body)
               → If success: update scope entry with ty = Some(morphism_type)
               → If failure (contains holes): leave ty as partial, record HasType constraint
            5. Update current_owner to HoleOwner::Def(name)
            6. Reset current_ordinal to 0

        (rule lhs rhs meta) →
            1. Set current_owner to HoleOwner::Rule { rule_index }
            2. Reset current_ordinal to 0
            3. Elaborate lhs via typed_sexpr_to_morphism(lhs)
            4. Elaborate rhs via typed_sexpr_to_morphism(rhs)
            5. Generate RuleBoundary constraints:
               source(lhs) must equal source(rhs)
               target(lhs) must equal target(rhs)
            6. Compile rule as before

        (begin forms...) →
            1. Push new TypedScope
            2. Recurse on forms
            3. Pop scope
```

**Phase 2: Typed term elaboration** (`typed_sexpr_to_morphism`)

This replaces the static `sexpr_to_hyperedge` for the elaboration path:

```
typed_sexpr_to_morphism(expr: &SExpr) -> Result<MorphismTerm, ElaborationError>:
    match expr:
        Atom(Symbol(sym)) if sym.starts_with('?') →
            // User hole — create metavariable
            let name = sym.trim_start_matches('?')
            let metavar_id = fresh_metavar()
            let local_ctx = snapshot_current_context()
            let expected_type = infer_expected_type_from_position()
            record MetavarInfo { id, name, owner, ordinal, local_ctx, expected_type, span }
            emit TraceNode::MetavarIntroduced { metavar_id, expected_type }
            current_ordinal += 1
            return Ok(MorphismTerm::Hole(metavar_id_to_hole_id(metavar_id)))

        Atom(Symbol(sym)) →
            // Bare symbol — look up in scope
            if let Some(entry) = scope_lookup(sym):
                // Known binder — return as generator reference
                return Ok(make_generator_term(entry))
            else:
                // Unknown symbol — this is the existing UnboundSymbol error path
                // In the "loose" elaboration mode, fall through to placeholder
                return Ok(MorphismTerm::Hole(hole_name_to_id(sym)))

        List(items) →
            // Composition — elaborate each component, generate boundary constraints
            let mut components = Vec::new()
            for (i, item) in items.iter().enumerate():
                let term = typed_sexpr_to_morphism(item)?
                components.push(term)

            // Generate boundary constraints for adjacent components
            for i in 0..components.len()-1:
                let left_type = try_infer_type(&components[i])
                let right_type = try_infer_type(&components[i+1])
                if let (Some(lt), Some(rt)) = (left_type, right_type):
                    emit_constraint(ObjectEq {
                        lhs: ObjectConstraint::Known(lt.target),
                        rhs: ObjectConstraint::Known(rt.source),
                    }, CompositionBoundary { left_index: i, right_index: i+1 })

            return Ok(MorphismTerm::Compose { components, inputs, outputs, doctrine: None })
```

**Phase 3: Constraint solving**

After all forms are elaborated, run the constraint solver:

```
solve_constraints():
    let worklist: VecDeque<ConstraintId> = all constraints
    let max_iterations = constraints.len() * 2  // fuel limit

    for _ in 0..max_iterations:
        if worklist.is_empty(): break

        let cid = worklist.pop_front()
        let constraint = &constraints[cid]

        match constraint.kind:
            ObjectEq { lhs, rhs } →
                let lhs' = subst.apply_object(lhs)
                let rhs' = subst.apply_object(rhs)
                match (lhs', rhs'):
                    (Known(a), Known(b)) if a == b → // satisfied, remove
                    (Known(a), Known(b)) →
                        emit TraceNode::UnificationFailure
                        mark affected metavars as Inconsistent
                    (Metavar(m), Known(obj)) | (Known(obj), Metavar(m)) →
                        subst.object_map.insert(m, obj)
                        emit TraceNode::UnificationStep
                        re-enqueue constraints mentioning m
                    (Metavar(m1), Metavar(m2)) →
                        // Tie-break: bind larger to smaller
                        let (hi, lo) = if m1 > m2 { (m1, m2) } else { (m2, m1) }
                        subst.object_map.insert(hi, /* redirect to lo */)

            HasType { metavar, expected } →
                if subst.is_solved(metavar):
                    let term = subst.get_morphism(metavar)
                    try typing_ctx.check_morphism(term)
                    // verify against expected
                else:
                    // Record expected type on metavar info
                    // Re-enqueue if any dependency is solved later

            SourceEq { metavar, object } →
                similar to ObjectEq

            TargetEq { metavar, object } →
                similar to ObjectEq

    // Finalize: classify each metavariable
    for metavar in &metavars:
        if subst.is_solved(metavar.id):
            metavar.status = Solved(subst.get_morphism(metavar.id).clone())
        else if has_inconsistent_constraints(metavar.id):
            metavar.status = Inconsistent(collect_conflicts(metavar.id))
        else if depends_on_unsolved(metavar.id):
            metavar.status = Blocked(find_blocking_metavar(metavar.id))
        else:
            metavar.status = Unsolved
```

**Phase 4: Result packaging**

```rust
let proof_state = ProofState {
    goals: metavars.iter().map(|m| m.to_goal_state(&subst, &trace)).collect(),
    constraints: constraints.clone(),
    subst: subst.clone(),
    trace: trace.clone(),
};
```

### 3.4 Tricky cases

**Two holes with same name in same owner:**
```lisp
(def foo (compose ?x ?x))
```
Each `?x` gets a distinct `MetavarId` (ordinal 0 and 1). They are NOT unified by name. If the user wants them to be the same, they must use a single `?x` and reference it — but since Mac Lane is first-order, this is just "two independent holes that happen to have the same name." A lint can warn about this.

**Bare symbols becoming kernel placeholders:**
In the typed elaborator, bare symbols are looked up in scope first. If found, they become generator references (a `MorphismTerm::Generator` or a reference to the definition). If NOT found, the existing behavior is `UnboundSymbol` error. The static `sexpr_to_hyperedge` helper (used for meta/rule bodies that bypass full elaboration) is unchanged — it continues to produce `Hole(hash)` for bare symbols. These are **not** metavariables and are not tracked in `ProofState`.

**Macro-generated code provenance:**
The existing `Expander` does not track invocation spans on expanded forms. The plan records `span: None` for any metavariable introduced inside macro-expanded code. A future enhancement (outside this plan's scope, but compatible with the data model) would thread `invocation_span` through `apply_template`.

**Reject terms in typed setting:**
`MorphismTerm::Reject` is ill-typed by definition. When the elaborator encounters a Reject, it:
1. Records a diagnostic (existing behavior).
2. Does NOT create a metavariable.
3. Skips constraint generation for that subtree.
4. The trace records a `UnificationFailure` node if a Reject appears where a typed term is expected.

---

## 4. Goal / ProofState API for Consumers

### 4.1 ProofState

```rust
/// Complete proof state after elaboration + constraint solving.
/// This is the kernel's authoritative output for downstream tools.
#[derive(Debug, Clone)]
pub struct ProofState {
    /// All goals (one per metavariable).
    pub goals: Vec<GoalState>,
    /// All constraints generated during elaboration.
    pub constraints: Vec<Constraint>,
    /// Current substitution (partial — unsolved metavars not present).
    pub subst: MetaSubst,
    /// Explanation trace.
    pub trace: ElaborationTrace,
}
```

### 4.2 GoalState

```rust
/// A single goal (metavariable) and its full state.
#[derive(Debug, Clone)]
pub struct GoalState {
    /// Unique goal ID (= metavariable ID).
    pub id: MetavarId,
    /// User-facing name (from `?name`).
    pub name: String,
    /// Owner: which definition/rule/top-level form introduced this goal.
    pub owner: HoleOwner,
    /// Source span where the hole appears.
    pub span: Option<Span>,
    /// Typed local context at the introduction point.
    pub local_context: LocalContext,
    /// What this goal must inhabit.
    pub expected_type: ExpectedType,
    /// Current status.
    pub status: GoalStatus,
    /// Constraint IDs relevant to this goal.
    pub relevant_constraints: Vec<ConstraintId>,
    /// Summary of why this goal is in its current status.
    /// Derived from the trace — not stored redundantly.
    pub status_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalStatus {
    Unsolved,
    Solved(MorphismTerm),
    Blocked { depends_on: MetavarId },
    Inconsistent { conflicts: Vec<ConstraintId> },
}

/// From the existing HOLE_OCCURRENCE_IMPLEMENTATION_PLAN.md
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HoleOwner {
    Def(String),
    Rule { rule_index: u32 },
    TopLevel { form_index: u32 },
}
```

### 4.3 Serialization boundary

For `WorkspaceReport`-like transport, `ProofState` needs a lightweight summary:

```rust
/// Lightweight proof state summary for WorkspaceReport transport.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProofStateSummary {
    pub goal_count: usize,
    pub solved_count: usize,
    pub unsolved_count: usize,
    pub blocked_count: usize,
    pub inconsistent_count: usize,
    pub goals: Vec<GoalSummary>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GoalSummary {
    pub id: u32,
    pub name: String,
    pub owner: String,
    pub status: String,
    pub target_display: String,
    pub context_display: Vec<String>,
}
```

`ProofStateSummary` is derived from `ProofState` by the kernel. Consumers that need full detail use `ProofState` directly; those that need only display strings use the summary.

---

## 5. Pretty-Printing Hooks (Kernel-Side Only)

### 5.1 Types requiring stable pretty-printing

| Type | Format requirement | Example |
|------|-------------------|---------|
| `MetavarId` | `?0`, `?1`, etc. | `?0` |
| `ExpectedType` | `source → target` | `A → B` |
| `ObjectConstraint` | Object name or `?m` | `A` or `?3` |
| `ContextEntry` | `name : type` | `f : A → B` |
| `LocalContext` | Comma-separated entries | `[x : A, f : A → B]` |
| `GoalStatus` | Status word + detail | `unsolved`, `solved = f ∘ g` |
| `Constraint` | LHS = RHS (provenance) | `source(?0) = A (composition at line 4)` |
| `ElaborationTrace` (summary) | Bullet list of key events | `- ?0 introduced at line 3\n- solved ?0 = f` |

### 5.2 Determinism requirements

- Metavariable names: always `?N` where N is the `MetavarId` index. User-given names appear in parentheses: `?0 (x)`.
- Object names: use the generator name from `TypingContext` if available; fall back to `obj#N`.
- Constraint ordering: always by `ConstraintId` (allocation order).
- Context entries: always in scope order (outermost first).
- Elided sections: if context has >5 entries, show first 3 + "..." + last 1. Cutoff is configurable.

### 5.3 Format functions

```rust
impl MetavarId {
    pub fn display_name(&self) -> String { format!("?{}", self.0) }
}

impl ExpectedType {
    pub fn display(&self, naming: &dyn ObjectNaming) -> String { ... }
}

impl LocalContext {
    pub fn display(&self, naming: &dyn ObjectNaming, max_entries: usize) -> String { ... }
}

impl GoalState {
    pub fn display_summary(&self, naming: &dyn ObjectNaming) -> String { ... }
}

/// Trait for resolving object IDs to display names.
pub trait ObjectNaming {
    fn object_name(&self, id: ObjectId) -> String;
    fn generator_name(&self, id: GeneratorId) -> String;
}
```

---

## 6. Testing & Verification Strategy

### 6.1 Unit tests: MetavarId stability/determinism (8 tests)

```
test_metavar_ids_sequential
test_metavar_ids_deterministic_across_runs
test_same_name_different_metavars
test_different_owners_independent_ordinals
test_metavar_id_display
test_metavar_id_ordering
test_two_holes_same_name_distinct_ids
test_reset_ordinal_on_owner_change
```

### 6.2 Unification tests (10 tests)

```
test_object_eq_same_object_trivial
test_object_eq_different_objects_fail
test_object_eq_metavar_solved
test_object_eq_two_metavars_tiebreak
test_source_eq_constraint_solved
test_target_eq_constraint_solved
test_has_type_constraint_checked
test_composition_boundary_constraints_generated
test_cyclic_constraint_detected
test_inconsistent_constraints_reported
```

### 6.3 "Goal target exists" tests (8 tests)

```
test_single_hole_has_goal
test_hole_in_def_has_owner
test_hole_in_rule_has_rule_owner
test_hole_in_composition_has_boundary_type
test_hole_in_nested_begin_has_full_context
test_multiple_holes_all_have_goals
test_bare_symbol_is_not_a_goal
test_reject_term_is_not_a_goal
```

### 6.4 Trace tests (6 tests)

```
test_trace_records_metavar_introduction
test_trace_records_constraint_generation
test_trace_records_unification_step
test_trace_records_unification_failure
test_trace_records_blocked_status
test_trace_has_causal_edges
```

### 6.5 Snapshot tests (8 tests)

Using `insta` crate for snapshot testing:

```
test_snapshot_simple_def_goal_summary
test_snapshot_composition_goals
test_snapshot_rule_with_holes
test_snapshot_unsolved_goal_display
test_snapshot_solved_goal_display
test_snapshot_inconsistent_goal_display
test_snapshot_full_proof_state
test_snapshot_trace_summary
```

### 6.6 Property tests (4 tests)

```
test_substitution_idempotence    // σ(σ(t)) = σ(t) for random σ, t
test_occurs_check_prevents_cycles  // no σ where ?m = f(?m)
test_solved_constraints_satisfied  // if solved, σ satisfies all constraints
test_metavar_ordering_stable       // BTreeMap iteration = allocation order
```

### 6.7 Regression tests (4 tests)

```
test_macro_expanded_hole_has_none_span
test_bare_symbol_not_recorded_as_metavar
test_empty_composition_rejected
test_duplicate_def_rejected_before_constraint_gen
```

### 6.8 Integration test (2 tests)

```
test_proof_state_in_workspace_report  // WorkspaceReport carries ProofState
test_edgelord_consumes_goal_state     // EdgeLorD test reads goal from ProofState
```

**Total: 50 tests**

---

## 7. Implementation Gates

### Gate 0: Data model types (compiles, 0 behavioral change)

**Files to create/modify:**
- `new_surface_syntax/src/proof_state.rs` (NEW) — `MetavarId`, `MetavarInfo`, `LocalContext`, `ContextEntry`, `ExpectedType`, `ObjectConstraint`, `Constraint`, `ConstraintId`, `ConstraintKind`, `ConstraintProvenance`, `ConstraintReason`, `MetaSubst`, `TraceNode`, `TraceNodeId`, `TraceNodeKind`, `TraceEdge`, `ElaborationTrace`, `ProofState`, `GoalState`, `GoalStatus`, `HoleOwner`, `ProofStateSummary`, `GoalSummary`
- `new_surface_syntax/src/lib.rs` — add `pub mod proof_state;`, re-export key types
- `new_surface_syntax/Cargo.toml` — no new deps (all types use existing kernel types)

**Tests added:** 8 MetavarId unit tests, 4 MetaSubst property tests (12 total)

**API introduced:** All types above. No behavioral change to existing code.

**Migration:** None — new module, no existing code touched.

### Gate 1: TypedElaborator skeleton (compiles, delegates to old elaborator)

**Files to modify:**
- `new_surface_syntax/src/elaborate.rs` — add `TypedElaborator` struct alongside existing `Elaborator`. `TypedElaborator::elaborate()` initially delegates to `Elaborator::elaborate()` and returns an empty `ProofState`.
- `new_surface_syntax/src/lib.rs` — add `typed_elaborate_query()` function that calls `TypedElaborator`.

**Tests added:** 2 integration tests (proof_state_in_workspace_report, empty ProofState is valid) (2 total)

**API introduced:** `TypedElaborator::new()`, `TypedElaborator::elaborate()`, `typed_elaborate_query()`.

**Migration:** `compile_comrade_db` gains an optional `typed: bool` parameter. When `false`, uses old path. When `true`, uses `TypedElaborator`. Default: `false`.

### Gate 2: Metavariable introduction (holes become MetavarIds)

**Files to modify:**
- `new_surface_syntax/src/elaborate.rs` — `TypedElaborator::typed_sexpr_to_morphism()` creates `MetavarInfo` for each `?`-hole. Bare symbols still use old path.
- `new_surface_syntax/src/proof_state.rs` — `MetavarInfo` population logic.

**Tests added:** 8 "goal target exists" tests, 2 trace tests (10 total)

**API change:** `ProofState.goals` is now populated (but all goals are `Unsolved` with `ExpectedType::Morphism { source: Metavar, target: Metavar }`).

**Migration:** `MetavarId` is mapped to `HoleId` via `metavar_id.as_u32()` for backward compatibility with `MorphismTerm::Hole(HoleId)`. The kernel `HoleId` becomes a transport format; `MetavarId` is the authoritative identity.

### Gate 3: Context snapshots (local context captured at each hole)

**Files to modify:**
- `new_surface_syntax/src/elaborate.rs` — `TypedElaborator` maintains a `scope_stack: Vec<TypedScope>`. At each `?`-hole, snapshot `scope_stack` into `LocalContext`.
- `new_surface_syntax/src/proof_state.rs` — `LocalContext::display()`.

**Tests added:** 4 snapshot tests for context display (4 total)

**Migration:** None — additive.

### Gate 4: Constraint generation (boundary constraints from composition)

**Files to modify:**
- `new_surface_syntax/src/elaborate.rs` — After elaborating a composition, generate `ObjectEq` constraints for adjacent component boundaries. After elaborating a rule, generate `RuleBoundary` constraints.
- `new_surface_syntax/src/proof_state.rs` — Constraint allocation and storage.

**Tests added:** 6 constraint generation tests (6 total)

**API change:** `ProofState.constraints` is now populated.

### Gate 5: Constraint solver (first-order unification on ObjectConstraints)

**Files to modify:**
- `new_surface_syntax/src/solver.rs` (NEW) — `solve_constraints()` function. Reuses unification principles from `tcb_core::pattern::unify_patterns` but operates on `ObjectConstraint` / `MetavarId` level.
- `new_surface_syntax/src/elaborate.rs` — Call `solve_constraints()` after form elaboration.

**Tests added:** 10 unification tests, 4 snapshot tests (14 total)

**API change:** `ProofState.subst` is now populated. `GoalState.status` reflects solver results.

### Gate 6: Trace recording (full explanation trace)

**Files to modify:**
- `new_surface_syntax/src/elaborate.rs` — Emit `TraceNode` at each metavar introduction, constraint generation, and unification step.
- `new_surface_syntax/src/solver.rs` — Emit trace nodes during solving.
- `new_surface_syntax/src/proof_state.rs` — `ElaborationTrace::display_summary()`.

**Tests added:** 4 trace tests, 2 snapshot tests (6 total)

### Gate 7: Pretty-printing and summary (display functions)

**Files to modify:**
- `new_surface_syntax/src/proof_state.rs` — `impl Display` for all key types. `ProofStateSummary` generation. `ObjectNaming` trait.
- `new_surface_syntax/src/lib.rs` — Export display utilities.

**Tests added:** 4 snapshot tests (4 total)

### Gate 8: Wire it up (replace default elaboration path)

**Files to modify:**
- `new_surface_syntax/src/lib.rs` — `compile_comrade_db` uses `TypedElaborator` by default.
- `new_surface_syntax/src/comrade_workspace.rs` — `WorkspaceReport` gains `pub proof_state: Option<ProofState>`.
- `new_surface_syntax/src/core.rs` — `CoreBundleV0` gains `pub proof_state: Option<ProofState>`, with `Default` impl setting it to `None`.

**Tests added:** 2 integration tests, 2 regression tests (4 total)

**Migration:** Existing consumers that don't use `proof_state` are unaffected (it's `Option`).

### Gate summary

| Gate | Tests | Cumulative | Description |
|------|-------|-----------|-------------|
| 0 | 12 | 12 | Data model types |
| 1 | 2 | 14 | TypedElaborator skeleton |
| 2 | 10 | 24 | Metavariable introduction |
| 3 | 4 | 28 | Context snapshots |
| 4 | 6 | 34 | Constraint generation |
| 5 | 14 | 48 | Constraint solver |
| 6 | 6 | 54 | Trace recording |
| 7 | 4 | 58 | Pretty-printing |
| 8 | 4 | 62 | Wire it up |

---

## 8. Performance & Determinism Plan

### 8.1 Complexity

| Operation | Complexity | Bound |
|-----------|-----------|-------|
| Metavar allocation | O(1) per hole | N metavars total |
| Context snapshot | O(S) per hole | S = scope depth |
| Constraint generation | O(C) per composition | C = components |
| Constraint solving | O(K * N) worst case | K constraints, N metavars |
| Occurs check | O(T) per binding | T = term size |
| Trace recording | O(1) per event | |
| Full elaboration | O(F * (S + C + K*N)) | F = forms |

For typical Mac Lane files (F < 100, S < 20, C < 10, K < 50, N < 30), this is well under 1ms.

### 8.2 Fuel limit

The constraint solver has a fuel limit of `constraints.len() * 2` iterations. If exhausted, remaining unsolved metavars are marked `Unsolved` (not `Blocked` — we don't know the cause). This prevents pathological cases without lying about the result.

### 8.3 Deterministic ordering

- All maps: `BTreeMap` (sorted by key).
- Metavar IDs: sequential allocation.
- Constraint IDs: sequential allocation.
- Trace nodes: append order.
- Unification tie-break: lower `MetavarId` wins.
- Goal list: sorted by `MetavarId`.

### 8.4 Caching/incremental

This plan does NOT implement incremental elaboration across edits. Each `did_change` triggers a full re-elaboration. This is acceptable because:
- Elaboration is fast (< 1ms for typical files).
- The debounce mechanism in EdgeLorD already coalesces rapid edits.
- Incremental elaboration is a future optimization that the data model supports (metavar IDs are stable within a snapshot).

---

## 9. Risks & Mitigations

### Risk 1: Interaction with macro expansion

**Problem:** Macro-expanded code may introduce holes that lack source spans. The trace would have gaps.

**Mitigation:** All spans are `Option<Span>`. Macro-expanded holes get `span: None`. The trace explicitly records `MetavarIntroduced` with `span: None`, which downstream tools can display as "from macro expansion." No fake spans.

### Risk 2: sexpr_to_hyperedge backward compatibility

**Problem:** The static helper `sexpr_to_hyperedge` is used by code paths outside the typed elaborator (e.g., `sexpr_to_morphism` for rule bodies). Changing it breaks those paths.

**Mitigation:** `sexpr_to_hyperedge` is NOT changed. The typed elaborator has its own `typed_sexpr_to_morphism` that creates metavariables. The static helper continues to produce `Hole(hash)` for backward compatibility. Gate 1 explicitly delegates to the old path first.

### Risk 3: CoreBundleV0 bloat

**Problem:** Adding `ProofState` to `CoreBundleV0` increases memory usage and serialization size.

**Mitigation:** `proof_state` is `Option<ProofState>`. The `Default` impl sets it to `None`. Consumers that don't need it never pay for it. The `ProofStateSummary` provides a lightweight alternative for transport.

### Risk 4: MetavarId-to-HoleId mapping

**Problem:** Existing code uses `HoleId = u32` (FNV hash). New code uses `MetavarId(u32)` (sequential). These are different number spaces.

**Mitigation:** The `MetavarId` is mapped to `HoleId` via a simple `metavar_id.as_u32()` cast. Since `HoleId` is only used as an opaque tag within `MorphismTerm::Hole`, and the old FNV hashes are in a completely different numeric range (2 billion+) from sequential IDs (0, 1, 2...), there is no collision in practice. A debug assertion verifies this at gate 2.

### Risk 5: TypingContext population

**Problem:** The current elaborator does not populate a `TypingContext`. The typed elaborator needs one to infer types, but the kernel's `TypingContext` expects registered generators with `GeneratorId`s that don't exist at elaboration time.

**Mitigation:** The typed elaborator maintains a **lightweight typing context** that maps binder names to their inferred types (from `def` bodies), not a full kernel `TypingContext`. Full kernel typing is a post-elaboration step. The elaborator's type inference is best-effort: if it can't determine a type, it records `ExpectedType::Morphism { source: Metavar(_), target: Metavar(_) }` and generates constraints. This is honest and correct.

### Risk 6: Test golden file breakage

**Problem:** Changing elaboration output may break golden tests in `new_surface_syntax`.

**Mitigation:** The typed elaborator is opt-in (Gate 1: `typed: bool` parameter). Golden tests continue to use the old path. New golden tests are added for the typed path. Gate 8 switches the default only after all new tests pass.

---

## Appendix: File Touch List

### New files
| File | Gate | Contents |
|------|------|----------|
| `new_surface_syntax/src/proof_state.rs` | 0 | All data model types |
| `new_surface_syntax/src/solver.rs` | 5 | Constraint solver |
| `new_surface_syntax/tests/typed_elaboration.rs` | 2+ | All new tests |

### Modified files
| File | Gate | Change |
|------|------|--------|
| `new_surface_syntax/src/lib.rs` | 0,1,8 | Module declaration, typed entry point, default switch |
| `new_surface_syntax/src/elaborate.rs` | 1-6 | TypedElaborator alongside Elaborator |
| `new_surface_syntax/src/core.rs` | 8 | `proof_state: Option<ProofState>` on CoreBundleV0 |
| `new_surface_syntax/src/comrade_workspace.rs` | 8 | `proof_state` on WorkspaceReport |
| `new_surface_syntax/Cargo.toml` | 0 | (no new deps expected) |

### Untouched files
| File | Reason |
|------|--------|
| `tcb_core/src/ast/terms.rs` | MorphismTerm::Hole(HoleId) unchanged |
| `tcb_core/src/pattern/mod.rs` | Existing unification unchanged |
| `tcb_core/src/ast/typing.rs` | TypingContext unchanged |
| `tcb_core/src/doctrine.rs` | Doctrine system unchanged |
| `new_surface_syntax/src/expand.rs` | Macro expander unchanged |
| `new_surface_syntax/src/parser.rs` | Parser unchanged |

No changes to `tcb_core` are required. This plan is entirely within `new_surface_syntax`.
