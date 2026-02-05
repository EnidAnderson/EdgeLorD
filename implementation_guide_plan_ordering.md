# Implementation Guide: Navigating the Mac Lane Planning Docs

**Audience:** an implementing AI working in the repo  
**Goal:** explain *which* plan to follow *when*, and the **logical execution order** across plans.  
**Key idea:** treat the plans as a dependency graph (kernel proof-state → structured diagnostics → IDE/UX consumers).

---

## 1) The documents and what each one is for

### A. Kernel proof-state plan (systemic change; upstream of everything)
**Doc:** `Kernel Proof-State Elaboration Plan (Rewrite: “Real Proof Kernel”)` (v2)  
**Scope:** `new_surface_syntax` + `tcb_core` only (kernel)  
**Delivers:** typed metavariables + constraints + unification/solving + traceable explanations → `ProofState` API.

This is the **foundational** change. Almost every “smart” UX feature (typed goals, explain, intelligent fixes) becomes *easy* once this exists, and brittle or impossible if it doesn’t.

### B. World-class diagnostics / pretty-printing / linting plan (downstream consumers)
**Doc:** `World-Class Diagnostics, Pretty-Printing, and UX Plan` (v1)  
**Scope:** kernel + EdgeLorD + LSP + lint UX  
**Delivers:** `StructuredDiagnostic`, stable codes, rich spans, pretty printers, lint framework, explain surfaces.

This plan assumes the kernel can *authoritatively* say what a hole is and what it needs. If you implement this before the proof-state kernel, you’ll either:
- hardcode “unknown” types (wasted effort), or
- build duplicate “syntactic goal” logic (technical debt you later delete).

### C. Addendum(s) and small follow-on notes
You may have extra notes/patches (e.g., “deficiencies + preferred fixes”). Treat these as **clarifications** that refine a plan section, not as a new plan.

---

## 2) The dependency graph (what must come first)

### Hard dependencies (must be done first)
1. **Proof-state kernel (v2)** → provides typed holes, constraints, solver, trace.
2. **Pretty-print hooks for proof-state artifacts** (goal summaries, constraint summaries, trace summaries)  
   *Why:* explanations and diagnostics need stable printing to be testable.

### Soft dependencies (can be parallelized, but don’t finalize before proof-state)
- `StructuredDiagnostic` data model and code registry can be started early.
- Macro-span “no fake spans” fixes are helpful early (they improve kernel provenance even before UX consumers exist).

### Strongly downstream (implement after proof-state is stable)
- Lints that depend on typing or scope snapshots.
- LSP/hover/inlay improvements that display goal targets/types.
- Diagnostic-driven code actions that rely on kernel explanations.

---

## 3) The recommended execution order (the “correct” roadmap)

### Phase 0 — Lock the single source of truth
**Rule:** from this point onward, anything that needs goal/type/explain must consume **kernel ProofState** (or a kernel-produced summary), *not* re-derive from syntax.

Concrete action:
- Add `ProofState` (and/or `ProofStateSummary`) to the compile output surface (workspace report/bundle), but keep it optional until stable.

### Phase 1 — Implement the proof-state kernel plan end-to-end (v2)
Follow the v2 gate sequence (names may differ in your repo layout, but the order matters):

1. **Gate 0:** Data model + meta IR compiles  
   - `ObjMetaId`, `MorMetaId`, `ObjExpr`, `MorExpr`  
   - constraints, substitution, trace types
2. **Gate 1:** Bidirectional elaboration skeleton (no solving)  
   - allocate metas from `?name` only  
   - generate constraints from typing rules (no “position guessing”)  
   - produce `ProofState` with unsolved goals + constraints
3. **Gate 2:** Object unification solver (endpoints only)  
   - solve `ObjEq` and propagate types/endpoints
4. **Gate 3:** Morphism-term unification solver (the “real spine”)  
   - solve `MorEq` by binding `Meta(m) := term` with occurs-check  
   - structural unify `Compose`/`App`/`Gen` heads
5. **Gate 4:** Dependency analysis + blocked classification  
   - compute mentions sets, blocked chains, and minimal conflict sets
6. **Gate 5:** Trace DAG + explanation queries  
   - derivation → constraints → solve steps → conflicts  
   - implement `explain_goal` / `explain_conflict`
7. **Gate 6:** Zonking + lowering to `tcb_core::MorphismTerm`  
   - keep metas explicit in `ProofState`; decide placeholder mapping only for lowered bundle
8. **Gate 7:** Make `ProofState` authoritative output  
   - compile path returns (bundle, proof_state) consistently  
   - add at least one consumer integration test that reads goal targets

**Stop condition for Phase 1:** you can open a file with holes and the kernel reports:
- stable `MorMetaId`s,
- expected types/endpoints when derivable,
- constraints,
- blocked vs unsolved vs inconsistent with an explanation trace.

### Phase 2 — Structured diagnostics as a kernel output format
Now execute the diagnostics plan gates, but with a key adjustment:

- Diagnostics should be able to reference `ProofState`:
  - “unsolved goal” diagnostics come from kernel goals
  - “explain available” is true when `explain_goal` exists and is non-empty
  - related spans can include goal origin spans, constraint provenance spans, etc.

Recommended order:
1. Introduce `StructuredDiagnostic` and stable code registry (kernel-side).
2. Replace stringly errors with `to_diagnostic()` conversions.
3. Add printers for kernel/proof-state terms used in messages (avoid `Debug` dumps).
4. Enforce “no fake spans” (CI grep + tests).

### Phase 3 — Lints (typed, after kernel proof-state exists)
Only implement lints once:
- scope snapshots are trustworthy, and
- goal targets are not “unknown” by default.

Implement lints in ascending sophistication:
1. purely syntactic (unused touch, duplicate touch, redundant begin, etc.)
2. goal-aware (suspicious holes, repeated hole names)
3. type-aware (shadowing with type mismatch, rule identity under definitional equality)

### Phase 4 — EdgeLorD/LSP consumption (last)
At this stage:
- EdgeLorD should map kernel `StructuredDiagnostic` to LSP `Diagnostic`.
- Hover/inlay should display **kernel goal state** (not syntactic fallbacks), except in parse-failure situations.

Implementation notes:
- Keep a “graceful degradation” path: when kernel parse fails, show syntactic info; when kernel succeeds, show kernel proof-state.

---

## 4) How to read the plans without thrashing

### “Plan dominance” rules
If two plans disagree, use this precedence order:

1. **Proof-state kernel v2** (systemic truth; it defines what holes *are*)
2. Addenda that explicitly correct a section
3. Diagnostics/pretty-print/lint plan v1 (consumer layer)

### Don’t optimize downstream too early
Avoid polishing:
- hover type strings,
- code actions,
- lint quick-fixes,
until proof-state has stable IDs and at least endpoint typing.

### One-file loop for implementing AI
To avoid cross-cutting churn:
- pick **one file** and get it compiling + tested,
- then move to the next compile error,
- keep changes minimal and reversible per gate.

---

## 5) Practical “checkpoints” for an implementing AI

After each gate, verify these checks before proceeding:

- `cargo check` passes (or the specific crate target you’re editing).
- A small focused test suite passes (add tests *as you go*).
- Determinism snapshot tests produce stable output (goal summaries, traces).
- No fake spans: new code never introduces placeholder spans.

Suggested tiny acceptance programs to keep in `tests/fixtures/`:
- single hole in a def: `(touch f) (def f ?h)`
- composition with hole: `(def x (compose ?f g))`
- rule with hole: `(rule (compose ?f g) (compose g ?f) (meta ...))`
- inconsistent boundary: force `A ≠ B` and ensure conflict is explained

---

## 6) What “done” means across the whole effort

You’re finished with the *systemic* part when:
- kernel proof-state exists, is deterministic, and can explain blocked/conflicts;
- diagnostics can render proof-state-derived messages without debug dumps;
- EdgeLorD shows typed goals and “explain” from kernel, not from syntax.

Everything else (extra lints, polish, more code actions) becomes incremental work.

---

## Appendix: Suggested filenames in the repo

- `docs/plans/kernel_proof_state_elaboration_plan_v2.md` (foundation)
- `docs/plans/world_class_diagnostics_plan_v1.md` (consumers)
- `docs/plans/implementation_guide_plan_ordering.md` (this file)

