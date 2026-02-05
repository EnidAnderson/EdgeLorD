# Kernel Proof-State Elaboration Plan (Rewrite: “Real Proof Kernel”)

**Status:** v2 (rewritten for implementation realism)  
**Scope:** **Kernel only** (`new_surface_syntax` + `tcb_core`) — *no* EdgeLorD/LSP/lint planning  
**Goal:** Make the elaborator a true **typed proof-state kernel**: **typed metavariables + constraints + unification/solving + traceable explanations**. Downstream UX consumes this state.

---

## 0. Executive Summary

### The core change

Today, elaboration produces `MorphismTerm` with `Hole(u32)` placeholders that are **untyped**, context-free, and semantically ambiguous (hash-holes vs “real goals”). That blocks:

- typed goals (“what does this hole need?”),
- principled explain-why (“why is it blocked?”),
- solver-driven fixes or proof search,
- and deterministic, authoritative proof-state outputs.

This plan makes metavariables **first-class kernel terms** and introduces a real elaboration state:

1. **Meta-terms** (metavariables for morphisms and objects) in an internal representation.
2. **Bidirectional elaboration** (check/infer) that generates constraints with provenance.
3. **Constraint solving** that can solve metavariables **to terms**, not just endpoints.
4. **Zonking** (substitution application) to produce final kernel terms + a complete `ProofState`.
5. **Traceable explanations** from a structured derivation + solver trace graph.

> Downstream diagnostics/lints/LSP become consumers of `ProofState` instead of re-deriving intent.

---

## 1. Requirements & Non-Negotiable Invariants

### INV-1: Two meta namespaces (no conflation)
There must be distinct IDs:

- `ObjMetaId` — object metavariables
- `MorMetaId` — morphism metavariables

Never reuse the same ID space for both. This prevents category errors in occurs-check, substitutions, and traces.

### INV-2: Metavariables are **not** encoded as `HoleId`
Metavariables are represented **explicitly** in an internal meta-term IR (see §2). `MorphismTerm::Hole(HoleId)` remains legacy/transport/placeholder-only.

### INV-3: Determinism
Same input text → same:

- meta-id allocation order,
- constraint allocation order,
- solver behavior (tie-breaking rules),
- and trace rendering (stable ordering).

### INV-4: Honest provenance (no fake spans)
All provenance uses `Option<Span>`. Never fabricate `Span::new(0,0)`. Macro-generated code may have `None` spans unless/ until macro-expansion provenance is threaded.

### INV-5: Soundness boundary (“never lie”)
If the solver reports `Solved`, the substitution satisfies all constraints. If the system cannot prove a meta solved, it stays `Unsolved` or `Blocked`, not “solved-ish”.

### INV-6: Stable goal identity
Each hole occurrence in source becomes a distinct morphism metavariable, even if names match. “Same-name implies same meta” is **not** assumed. (A lint may warn about repeated names.)

---

## 2. Core Representations

### 2.1 Meta IDs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjMetaId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MorMetaId(u32);
```

Allocated sequentially in a deterministic walk.

---

### 2.2 Internal meta-term IR

We introduce a kernel-internal representation used **only during elaboration & solving**:

```rust
pub enum ObjExpr {
    Known(ObjectId),
    Meta(ObjMetaId),
}

pub enum MorExpr {
    // Rigid references:
    Gen(GeneratorId),
    Ref(String),            // before name resolution finalizes, if needed
    // Structured:
    Compose(Vec<MorExpr>),
    App { op: GeneratorId, args: Vec<MorExpr> },
    InDoctrine { doctrine: DoctrineKey, term: Box<MorExpr> },
    // Meta:
    Meta(MorMetaId),
}
```

**Notes**
- This does *not* require changing surface syntax.
- This may or may not require changing `tcb_core::MorphismTerm`. The safest path: keep `MorphismTerm` unchanged and add `MorExpr` only in `new_surface_syntax`, then “zonk + lower” to `MorphismTerm`.

---

### 2.3 Types and judgments

We keep the kernel’s intended typing discipline: morphisms have **object endpoints**.

```rust
pub struct MorType {
    pub src: ObjExpr,
    pub dst: ObjExpr,
}
```

Bidirectional elaboration uses judgments:

- `Γ ⊢ e ⇐ A`  (check `e` against expected type `A`)
- `Γ ⊢ e ⇒ (e', A)` (infer type `A` and elaborated term `e'`)

`Γ` is a typed context (see §2.4).

---

### 2.4 Typed local context

```rust
pub struct CtxEntry {
    pub name: String,
    pub ty: Option<MorType>,          // may be unknown early; becomes constrained
    pub def: Option<MorExpr>,         // optional definition body
    pub span: Option<Span>,
}

pub struct LocalContext {
    pub entries: Vec<CtxEntry>,       // outermost first, deterministic
    pub doctrine: Option<DoctrineKey>,
}
```

**Important:** the context used for a meta goal is a snapshot of the current elaboration scope stack. This is the basis for goal display, explanation, and later tactics.

---

## 3. Constraints

### 3.1 Constraint IDs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintId(u32);
```

---

### 3.2 Constraint kinds (term-level + type-level)

```rust
pub enum ConstraintKind {
    // Object equalities:
    ObjEq { a: ObjExpr, b: ObjExpr },

    // MorExpr typing constraints (bidirectional generates these):
    HasType { m: MorExpr, ty: MorType },

    // Morphism equalities (definitional/propositional equality in this kernel layer):
    MorEq { a: MorExpr, b: MorExpr },

    // Endpoint constraints for convenience (derived forms):
    SrcEq { m: MorExpr, src: ObjExpr },
    DstEq { m: MorExpr, dst: ObjExpr },
}
```

---

### 3.3 Provenance and “reason”

```rust
pub struct Constraint {
    pub id: ConstraintId,
    pub kind: ConstraintKind,
    pub span: Option<Span>,
    pub reason: ConstraintReason,
}

pub enum ConstraintReason {
    CompositionBoundary { left_i: usize, right_i: usize },
    AppArity,
    RuleBoundary,
    DefBody,
    HoleExpectedType,
    Inferred,
}
```

Constraints are allocated in elaboration order for determinism.

---

## 4. Substitution (“MetaSubst”) and Occurs Check

### 4.1 Separate substitutions

```rust
pub struct MetaSubst {
    pub obj: BTreeMap<ObjMetaId, ObjExpr>,
    pub mor: BTreeMap<MorMetaId, MorExpr>,
}
```

### 4.2 Core operations

- `apply_obj(ObjExpr) -> ObjExpr`
- `apply_mor(MorExpr) -> MorExpr`  (recursively)
- `compose(self, other) -> MetaSubst`
- `occurs_obj(meta, expr)` / `occurs_mor(meta, expr)`  
  Prevent cycles (`?m = f(?m)`).

### 4.3 Finalization (“zonk”)
After each binding, and at end of solve, we **zonk**:

- apply substitution to the RHS terms
- normalize substitution maps to be idempotent (`σ(σ(t)) = σ(t)`)

---

## 5. Bidirectional Elaboration (the real kernel algorithm)

### 5.1 Where expected types come from (no hand-waving)
Expected types are produced by the structure of kernel constructs:

- In `Compose([f, g])`, we require `dst(f) = src(g)`. If `f` or `g` is a meta, we generate endpoint metas and constraints.
- In `App(op, args)`, the operator’s signature yields expected types for args and result.
- In `Def name body`, `body` may be checked against a declared/expected type if present, else inferred with constraints.
- In `Rule lhs rhs meta`, we require `type(lhs) = type(rhs)` at endpoints.

### 5.2 Elaboration interface

```rust
pub fn infer(ctx: &mut ElabCtx, e: &SExpr) -> Result<(MorExpr, MorType), ElabError>;
pub fn check(ctx: &mut ElabCtx, e: &SExpr, expected: MorType) -> Result<MorExpr, ElabError>;
```

Where `ElabCtx` contains scope stack, meta allocators, constraint store, and trace.

### 5.3 Holes are introduced only by `?name`
When elaborating an atom symbol:

- if it starts with `?`: allocate `MorMetaId`, record a goal with:
  - local context snapshot
  - expected type if known (from `check`), else create fresh endpoint object metas and record constraints once structure requires them.
- if it is a bare symbol: resolve as rigid reference (scope lookup), else `UnboundSymbol` (no “loose mode” inside the proof kernel).

### 5.4 Composition elaboration (sketch)
To infer `(compose e1 e2 ... en)`:

1. Infer each `ei` to `(mi, ti: Ai → Bi)` where `Ai,Bi` may contain object metas.
2. For each adjacent pair, add `ObjEq { Bi = A(i+1) }` with provenance.
3. Return type `A1 → Bn`.

If any `ei` is `Meta(m)`, its type is unknown; we generate fresh object metas:
- `Meta(m)` gets type `α → β` and we add `HasType{ Meta(m), α→β }`.

This is how expected types propagate **without guessing**.

---

## 6. Solving: Unification + Constraint Propagation

### 6.1 Worklist solver (deterministic)
We solve constraints using a queue ordered by `ConstraintId` (allocation order). Re-enqueue affected constraints when new bindings are created.

### 6.2 Unifying objects
For `ObjEq(a,b)` after applying substitution:

- `Known(x) = Known(y)`:
  - success if equal else conflict
- `Meta(u) = t`:
  - bind `u := t` if occurs-check passes
- `Meta(u) = Meta(v)`:
  - tie-break: bind larger-id to smaller-id (or union-find) deterministically

### 6.3 Unifying morphism terms (the missing spine)
For `MorEq(a,b)` after substitution:

- If both are rigid structured terms, recursively unify:
  - `Compose(xs)` with `Compose(ys)` requires same length + pairwise unification
  - `App(op,args)` requires same `op` + args unify
  - `Gen(id)` requires same id
- If one side is `Meta(m)`:
  - bind `m := term` **if occurs-check passes**
  - but only if term is “acceptable” under your kernel’s equality (v1 = structural)
- If neither side is meta and heads mismatch: conflict

### 6.4 Solving typing constraints
`HasType{ m, ty }` is handled by turning it into endpoint constraints:

- If `m` can be inferred to have type `A→B`, add `ObjEq(A, ty.src)` and `ObjEq(B, ty.dst)`.
- If `m` is `Meta(mm)` and we have a stored “declared type” for that meta, unify the two types via object unification.
- If `m` is rigid but untypable given current signatures, produce a typed error (elaboration error) not a solver conflict.

### 6.5 Goal status classification
After saturation:

- `Solved`: `MorMetaId` bound in substitution to a term that typechecks (under the meta’s local context) **and** all constraints mentioning it are satisfied.
- `Blocked`: not bound, but constraints mention other unsolved metas (dependency analysis).
- `Unsolved`: not bound and no contradiction; constraints insufficient.
- `Inconsistent`: any constraint in its slice is conflicting.

### 6.6 Dependency graph (for “blocked” and explanations)
Maintain `mentions(m)` sets for each constraint:

- `constraint_mentions_obj_metas`
- `constraint_mentions_mor_metas`

Compute:
- `goal_depends_on(m) = metas in constraints(m) that are unsolved and not m`

Blocked reasons are derived from this graph, not guessed.

---

## 7. Traceable Explanations (Not Just Logging)

### 7.1 Trace is a causal DAG with stable node IDs

Nodes are one of:

- `Derive`: a typing/elaboration rule application
- `Constrain`: constraint emitted, linked to the derive node
- `Solve`: substitution binding performed, linked to the constraint(s) that forced it
- `Conflict`: failure with the two terms shown in pretty-printed normal form

Each node stores `span: Option<Span>` and stable references (meta IDs, constraint IDs).

### 7.2 Explanation queries (kernel API)
Provide deterministic query functions:

- `explain_goal(MorMetaId) -> ExplainReport`
- `explain_conflict(ConstraintId) -> ExplainReport`

Where `ExplainReport` is purely data + already pretty-printable.

This is the kernel’s “why” engine. Downstream just renders it.

---

## 8. Lowering/Zonking to existing kernel terms

After solving, we lower `MorExpr` → `tcb_core::MorphismTerm` by:

1. Apply full substitution (zonk).
2. Replace any remaining `Meta(m)` with:
   - either a stable placeholder (e.g. `MorphismTerm::Hole(HoleId::from_meta(m))`), **with a documented mapping**
   - or keep them only in proof-state and produce a partial core bundle if needed.

**Key policy:** metas remain explicit in `ProofState`; the lowered bundle is a separate artifact.

---

## 9. ProofState API (Kernel Output)

### 9.1 ProofState

```rust
pub struct ProofState {
    pub goals: Vec<GoalState>,
    pub constraints: Vec<Constraint>,
    pub subst: MetaSubst,
    pub trace: TraceDag,
}
```

### 9.2 GoalState

```rust
pub struct GoalState {
    pub id: MorMetaId,
    pub user_name: String,
    pub owner: HoleOwner,
    pub span: Option<Span>,
    pub local_context: LocalContext,
    pub expected: Option<MorType>,
    pub status: GoalStatus,
    pub relevant_constraints: Vec<ConstraintId>,
}
```

Goal identity is `MorMetaId`. Any mapping to old `HoleId` is secondary.

---

## 10. What requires changes in `tcb_core` vs `new_surface_syntax`

### 10.1 Recommended (minimal disruption) approach
- Keep `tcb_core::MorphismTerm` unchanged.
- Implement meta-term IR + solver + proof-state entirely in `new_surface_syntax`.
- Provide lowering/zonking functions that output `MorphismTerm`.

### 10.2 Optional “clean” approach (future)
Add a metavariable constructor to `tcb_core::MorphismTerm` (`Meta(MorMetaId)`) so the kernel term language can represent partial terms without encoding tricks. This is cleaner but broader.

This plan does **not** require that change.

---

## 11. Testing & Verification Strategy

### 11.1 Determinism tests
- meta allocation is stable across runs
- constraint allocation order stable
- solver tie-break stable
- trace rendering stable (snapshot tests)

### 11.2 Unification correctness tests
- object unification (meta/known/meta-meta)
- term unification (compose/app)
- occurs-check prevents cycles
- solver produces bindings that satisfy constraints

### 11.3 Typing + solving integration
- compositions propagate endpoints through metas
- rule boundary constraints force equal endpoints
- solved metas typecheck under local context

### 11.4 Explanation tests
- explain for blocked goal includes explicit dependency chain
- explain for conflict includes the minimal conflicting constraints and shows where they came from (span/reason)

### 11.5 Snapshot tests
- `GoalState` summaries
- `ProofState` summary
- `ExplainReport` for at least 5 representative scenarios

---

## 12. Implementation Gates (Practical, Kernel-Focused)

### Gate 0: Data model + meta-term IR compiles
- Add `ObjMetaId`, `MorMetaId`, `ObjExpr`, `MorExpr`, constraint structs, substitution structs, trace structs.
- No behavioral changes.

### Gate 1: Bidirectional elaborator skeleton (no solving)
- Implement `infer/check` that allocates metas and emits constraints but does not solve.
- Produce `ProofState` with unsolved goals and recorded constraints.

### Gate 2: Object unification solver (endpoints only)
- Implement solver for `ObjEq` and type unification.
- Goals now show typed endpoints even if terms unsolved.

### Gate 3: Term unification solver (real spine)
- Implement `MorEq` solving (`Meta(m) := term`) with occurs-check.
- Add basic structural unification for `Compose`/`App`.

### Gate 4: Dependency graph + blocked classification
- Compute mentions sets; derive `Blocked` precisely.

### Gate 5: Trace DAG + explanation queries
- Connect derivation → constraints → solver steps.
- Implement `explain_goal` and `explain_conflict`.

### Gate 6: Zonking + lowering to `MorphismTerm`
- Lower solved/unsolved metas consistently, without reusing legacy hash semantics.

### Gate 7: Make proof-state the authoritative output
- `compile` path returns bundle + proof-state.
- Any old “syntactic goal” mechanisms remain out-of-scope here.

---

## 13. Definition of Done (Kernel)

You have “the real elaborator/proof-state kernel” when:

1. Every `?name` yields a `MorMetaId` with a `GoalState` including local context and (when derivable) expected type.
2. Constraints are generated systematically from typing rules (no “infer_expected_type_from_position” hand-waving).
3. Solver can solve at least some metavars **to terms** via `MorEq` unification.
4. Blocked goals are computed from an explicit dependency graph.
5. Conflicts yield a minimal explanation with provenance.
6. ProofState is deterministic and snapshot-tested.
7. No fake spans anywhere in proof-state provenance or explanations.

---

## Appendix: File Touch Outline (Suggested)

### New
- `new_surface_syntax/src/proof_state.rs`
- `new_surface_syntax/src/meta_ir.rs`
- `new_surface_syntax/src/constraints.rs`
- `new_surface_syntax/src/solver.rs`
- `new_surface_syntax/src/trace.rs`
- `new_surface_syntax/tests/proof_state_kernel.rs`

### Modified
- `new_surface_syntax/src/elaborate.rs` (bidirectional elaboration entry points)
- `new_surface_syntax/src/lib.rs` (expose proof-state compile path)

No `tcb_core` changes are required for v1 of this kernel proof-state plan (only recommended for later cleanliness).


Addendum: Implementation-Critical Clarifications and Fixes

This addendum records a few kernel-semantics deficiencies in v2 that could otherwise cause implementation drift, underspecified behavior, or accidental reintroduction of “holes as mush.” Each item below states: (a) what’s underspecified, (b) why it matters, and (c) a preferred solution contract to adopt inside the kernel (still within scope: new_surface_syntax + tcb_core).

⸻

A1. Operator Signatures for App(op, args) Are Underspecified

Deficiency
The plan relies on: “the operator’s signature yields expected types for args and result,” but does not specify where these signatures come from, how they are loaded, or which environment is authoritative during elaboration.

Why it matters
Bidirectional elaboration cannot be deterministic (or correct) without an explicit signature environment. If signatures are implicitly fetched from “somewhere,” you’ll get inconsistent behavior across files, compilation phases, and future refactors.

Preferred solution
Define a Signature Environment that is the only source of truth for generator/operator typing during elaboration:
	•	Introduce a kernel-facing trait (or concrete struct) with a stable API:
	•	sig_of_generator(GeneratorId) -> GeneratorSig
	•	sig_of_generator_name(&str) -> Option<GeneratorSig> (optional convenience for early resolution)
	•	Where:

pub struct GeneratorSig {
    pub arg_tys: Vec<MorType>,     // expected types for args (each A_i → B_i or relevant form)
    pub result_ty: MorType,        // result type
}


	•	Construction rule (deterministic):
	1.	Load the ambient doctrine (if any) and import its generator signatures.
	2.	Extend with locally-introduced definitions that produce generators (if your language supports that).
	3.	Disallow mutation after elaboration begins (pure snapshot).
	•	Elaboration rule:
	•	For App(op, args), require args.len() == sig.arg_tys.len(); otherwise emit AppArity constraint/error with span.

Definition of done for A1
No part of the elaborator guesses operator argument types. All App checking is driven by SignatureEnv.

⸻

A2. Definitional Equality vs Structural Equality for MorEq Is Not Locked Down

Deficiency
The plan describes MorEq solving as “v1 structural,” but does not specify whether definitional equality (e.g. unfolding defs) participates in unification, and if so, how to keep it deterministic and terminating.

Why it matters
With purely structural equality, metas will rarely solve in the presence of named definitions, even when they are definitionally equal. Conversely, naïve unfolding can explode or become nonterminating.

Preferred solution
Adopt an explicit DefEq Policy with a deterministic, fuel-bounded normalizer hook. Keep it small in v1:
	•	Provide a kernel function:

normalize_mor(e: &MorExpr, env: &NormEnv, fuel: u32) -> MorExpr
normalize_obj(e: &ObjExpr, env: &NormEnv, fuel: u32) -> ObjExpr


	•	v1 normalization contract:
	•	δ-reduction (unfold named defs) is allowed only at rigid Ref(name) nodes that resolve to a def, and only up to fuel steps.
	•	No rewriting by rules in v1 (that’s a later layer).
	•	Composition/app trees are traversed deterministically left-to-right.
	•	Solver usage contract:
	•	Before attempting MorEq(a,b) structural unification, normalize both sides with a fixed fuel (e.g. 20) and record in the trace whether normalization changed either term.
	•	If fuel exhausts, record a trace node BlockedByNormalizationFuel (or similar) and leave the constraint unsolved rather than guessing.

Definition of done for A2
MorEq has a well-defined meaning in v1, is deterministic, and can solve metas through bounded unfolding without risking nontermination.

⸻

A3. “Zonk + Validate” Needs a Precise Contract (What Must Be Fully Known?)

Deficiency
The plan says “Solved metas must typecheck under local context,” but it doesn’t specify what counts as “typecheck” when object endpoints can themselves be meta, nor whether the kernel requires fully-resolved objects to report Solved.

Why it matters
Without a clear contract, you’ll either (a) incorrectly mark metas solved while still depending on unresolved object metas, or (b) never mark anything solved unless the whole file is fully determined. Both break user expectations and explanation quality.

Preferred solution
Define an explicit Validation Pass after constraint saturation:
	•	After solving, run:
	1.	Zonk: apply full substitution to every goal term and to every constraint.
	2.	Type Validation: for each MorMetaId with a substitution binding, check:
	•	Γ ⊢ term ⇐ expected_ty using the kernel’s type checker extended to accept ObjExpr endpoints.
	•	If expected_ty contains object metas, allow them only if they are constrained consistently by the current substitution.
	•	Classification rule:
	•	A goal is Solved(term) iff:
	1.	it has a mor binding in substitution,
	2.	occurs-check passed,
	3.	all constraints mentioning it are satisfied after zonk,
	4.	the goal term checks against its expected type in its LocalContext.
Otherwise:
	•	If it has a binding but fails constraints/typecheck → Inconsistent with conflict slice.
	•	If it has no binding but depends on unsolved metas → Blocked.
	•	Else → Unsolved.

Definition of done for A3
No goal is marked Solved unless it is provably consistent with constraints and typing under its local context (even if some unrelated metas remain unsolved elsewhere).

⸻

A4. Lowering Unresolved Metas Must Avoid Reintroducing Legacy HoleId Ambiguity

Deficiency
The plan says unresolved metas lower to MorphismTerm::Hole(HoleId::from_meta(m)) “with documented mapping,” but doesn’t specify how to guarantee non-collision with legacy hashed holes, nor how downstream prevents confusing these two classes.

Why it matters
If unresolved metas share the same numerical space as legacy hash holes, you reintroduce the original ambiguity (“is this a real goal or an accidental placeholder?”). Downstream tools will regress into heuristics.

Preferred solution
Introduce HoleId Domain Tags as a hard boundary:
	•	Reserve disjoint numeric ranges (domain tags) for:
	•	legacy hash holes (existing behavior)
	•	morphism metas
	•	object metas (if ever lowered, though ideally objects don’t lower as holes)
	•	Example mapping (conceptual contract; choose exact bits to match your existing HoleId conventions):
	•	HoleId::from_mor_meta(m) = 0x4D4F_0000 | (m.as_u32() & 0x0000_FFFF)  // “MO”
	•	HoleId::from_obj_meta(o) = 0x4F42_0000 | (o.as_u32() & 0x0000_FFFF)  // “OB”
	•	Additionally, require the kernel to include in ProofState:
	•	hole_origin(HoleId) -> HoleOrigin where HoleOrigin is one of:
	•	LegacyHashPlaceholder
	•	MorMeta(MorMetaId)
	•	ObjMeta(ObjMetaId)

Downstream never guesses: it queries origin.

Definition of done for A4
Lowered core terms cannot cause a “fake goal” interpretation. The mapping is deterministic, collision-safe by construction, and provenance is queryable.

⸻

A5. Ref(String) / Name Resolution Must Be Deterministic and “No Loose Mode”

Deficiency
The presence of MorExpr::Ref(String) implies a phase where names exist before resolution finalizes, but the plan doesn’t specify when resolution happens or what the failure behavior is. This is exactly where prior designs accidentally turned unbound symbols into holes.

Why it matters
If unresolved names can “temporarily” behave like metas or holes, you get nondeterminism and semantic leakage. The proof kernel must not silently coerce missing references into placeholders.

Preferred solution
Adopt an explicit Resolution Policy:
	•	During elaboration of an atom symbol:
	•	If it starts with ? → allocate a MorMetaId (the only hole path).
	•	Else → resolve immediately as a rigid reference:
	•	If found in scope/signature env → produce Gen(GeneratorId) (or a resolved rigid node).
	•	If not found → emit ElabError::UnboundSymbol { name, span } and do not produce a placeholder term.
	•	If you need Ref(String) for macro expansion staging or forward references, enforce:
	•	Ref(name) may exist only internally in the short window between parsing and “resolution finalize,” and must never reach solving.
	•	Before solver runs, run finalize_resolution():
	•	either resolves all Ref into rigid nodes
	•	or errors. No fallback to metas/holes.

Definition of done for A5
Unbound bare symbols are always hard errors in the proof kernel elaboration path. Only ?name makes goals.

⸻

Addendum Acceptance Checklist

This v2 plan + addendum is considered “implementation-tight” when:
	•	SignatureEnv exists and App typing uses only it (A1).
	•	MorEq unification has a stated DefEq/normalization policy and fuel (A2).
	•	“Solved” status is defined by an explicit zonk + validate pass (A3).
	•	Meta lowering uses domain-tagged HoleId mapping + origin queries (A4).
	•	Name resolution has no loose mode; unbound bare symbols never become holes (A5).

These addenda preserve the plan’s scope while closing the kernel-level semantic gaps that would otherwise cause complexity and expensive rework later.
