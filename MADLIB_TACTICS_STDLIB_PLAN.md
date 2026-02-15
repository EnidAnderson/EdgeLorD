# MADLIB_TACTICS_STDLIB_PLAN.md

## 1. MadLib Tactics Standard Library Plan

This document outlines the proposed directory structure and initial set of tactics for the MadLib tactics standard library. The goal is to provide a disciplined, deterministic, and sound foundation for interactive theorem proving within the Mac Lane ecosystem, respecting the hard constraints laid out in the `TACTICS_LAYER_SPEC.md`.

## 2. Directory Layout

The tactics standard library will reside under a new top-level directory within `madlib/`. The structure is designed for modularity, clarity, and future expansion, while keeping changes localized to avoid entangling existing systems.

```
madlib/
├─── prelude/             # Existing prelude
├─── doctrines/           # Existing doctrines
├─── facets/              # Existing facets
└─── tactics/             # NEW: Top-level directory for all tactics
    ├─── core/            # Core combinators, DSL components, infrastructure
    │    ├─── types.maclane        # Common types (e.g., SemanticPatch, TacticInput)
    │    ├─── dsl.maclane          # Tactic DSL primitives
    │    ├─── engine.maclane       # Internal runner/dispatch (if implemented in Mac Lane)
    │    └─── combinators.maclane  # Generic tactic combinators (seq, try, repeat)
    ├─── intro/           # Introduction tactics
    │    ├─── intro_binder.maclane   # Introduce a binder (touch)
    │    └─── intro_def.maclane      # Introduce a definition (def)
    ├─── exact/           # Exact matching/filling tactics
    │    └─── exact_term.maclane     # Fill a hole with an exact term
    ├─── rewrite/         # Rewrite tactics
    │    ├─── rewrite_rule.maclane   # Apply a single rewrite rule
    │    └─── rewrite_once.maclane   # Apply a rewrite once at a specified site
    ├─── simp/            # Simplification tactics
    │    ├─── simp_cfg.maclane       # Configuration for simplifier
    │    └─── simp_fuel.maclane      # Fuel-limited simplification (deterministic)
    ├─── cases/           # Case analysis tactics
    │    └─── cases_on_induction.maclane # Case analysis on inductive types
    ├─── search/          # Deterministic search/automation tactics
    │    ├─── search_bfs.maclane     # Breadth-first search (fuel-limited)
    │    └─── search_dfs.maclane     # Depth-first search (fuel-limited)
    ├─── utils/           # Utility tactics/helpers (e.g., introspection)
    │    └─── query_goal.maclane     # Introspect current goal
    └─── docs/            # User-facing documentation and examples
         ├─── README.md
         └─── examples.maclane
```

## 3. Initial Standard Tactics Set

This section defines an initial set of 10-20 tactics, focusing on core functionality and adherence to determinism, trust boundaries, and the `SemanticPatch` output model. Each tactic will be implemented as a Mac Lane macro or function that takes a `TacticInput` (as defined in `TACTICS_LAYER_SPEC.md`) and returns a `TacticResult`.

---

### Tactic Category: Core / Infrastructure

#### 3.1. `core.seq`

*   **Purpose:** Tactic combinator: applies tactics `T1` then `T2`.
*   **Input Expectations:** Takes two tactic expressions as arguments.
*   **Determinism Policy:** Inherits determinism from `T1` and `T2`. If `T1` fails, `seq` fails. If `T1` succeeds and `T2` fails, `seq` fails. Patches from `T1` applied before `T2`.
*   **Patch Emission:** Aggregates `SemanticPatch`es from `T1` and `T2` into a single `TacticResult::Success` or fails.
*   **Minimal Kernel Reliance:** Basic Mac Lane macro expansion/function application.

#### 3.2. `core.try`

*   **Purpose:** Tactic combinator: tries to apply a tactic `T`. If `T` fails, `try` succeeds without applying any patch.
*   **Input Expectations:** Takes one tactic expression.
*   **Determinism Policy:** Inherits determinism from `T`.
*   **Patch Emission:** If `T` succeeds, emits `T`'s `SemanticPatch`. If `T` fails, emits an empty `SemanticPatch` (representing no change).
*   **Minimal Kernel Reliance:** Basic Mac Lane macro expansion/function application.

#### 3.3. `core.repeat`

*   **Purpose:** Tactic combinator: applies tactic `T` zero or more times until it fails.
*   **Input Expectations:** Takes one tactic expression.
*   **Determinism Policy:** Inherits determinism from `T`. Repeats deterministically until `T` deterministically fails.
*   **Patch Emission:** Aggregates all `SemanticPatch`es from successful applications of `T`.
*   **Minimal Kernel Reliance:** Basic Mac Lane macro expansion/function application.

---

### Tactic Category: Introduction

#### 3.4. `intro.binder`

*   **Purpose:** Introduce a new binder (`touch`) into the current goal's context.
*   **Input Expectations:** `TacticInput` with `target_hole_id` or `target_goal_id` and an optional `name` hint for the new binder.
*   **Determinism Policy:** Deterministic:
    1.  Generates a unique, fresh, deterministic name if no name hint is provided (e.g., `_x_1`, `_x_2`).
    2.  Introduces the binder at a canonical, deterministic position (e.g., immediately after existing binders, or at the start of the goal context).
    3.  If a named hole exists, it introduces the binder into that hole's context.
*   **Patch Emission:** `SemanticPatch` with `PatchKind::IntroBinding { name, type_expr: "<unknown>" }` (type will be inferred by elaborator later).
*   **Minimal Kernel Reliance:** `touch` primitive.

#### 3.5. `intro.def`

*   **Purpose:** Introduce a new definition (`def`) into the current goal's context or at the module level.
*   **Input Expectations:** `TacticInput` with an optional `name` and `type_hint`.
*   **Determinism Policy:** Deterministic name generation and insertion position if not specified.
*   **Patch Emission:** `SemanticPatch` with `PatchKind::IntroDef { name, type_expr, value_expr: "<hole>" }`.
*   **Minimal Kernel Reliance:** `def` primitive.

---

### Tactic Category: Exact Matching / Filling

#### 3.6. `exact.term`

*   **Purpose:** Fill the current focused hole/goal with a provided Mac Lane term.
*   **Input Expectations:** `TacticInput` with `target_hole_id` or `target_goal_id` and `additional_args` containing `term: String` (the Mac Lane surface form).
*   **Determinism Policy:** Deterministic. Fails if the provided term does not exactly match the expected type of the hole/goal.
*   **Patch Emission:** `SemanticPatch` with `PatchKind::FillHole { hole_id, term }`.
*   **Minimal Kernel Reliance:** `quote`/`unquote` for term parsing, elaborator's type checking.

---

### Tactic Category: Rewrite

#### 3.7. `rewrite.rule`

*   **Purpose:** Apply a named rewrite rule to the entire focused goal or a sub-expression within it.
*   **Input Expectations:** `TacticInput` with `target_hole_id` or `target_goal_id`, `additional_args` containing `rule_id: String` (name of the rewrite rule) and `direction: RewriteDirection`. Optional `target_span: ByteSpan` for sub-expression targeting.
*   **Determinism Policy:**
    1.  Deterministic rule lookup by `rule_id`.
    2.  If `target_span` is provided, applies only at that exact location.
    3.  If no `target_span`, applies to the "topmost" matching occurrence in a canonical traversal order (e.g., pre-order traversal of the term AST).
    4.  Fails if the rule is not applicable.
*   **Patch Emission:** `SemanticPatch` with `PatchKind::Rewrite { rule_id, direction, target_span }`.
*   **Minimal Kernel Reliance:** `rule` primitive, elaborator's unification/matching engine.

#### 3.8. `rewrite.once_at_span`

*   **Purpose:** Apply a named rewrite rule exactly once at a specified byte span.
*   **Input Expectations:** `TacticInput` with `target_hole_id` or `target_goal_id`, `additional_args` containing `rule_id: String`, `direction: RewriteDirection`, and `target_span: ByteSpan`.
*   **Determinism Policy:** Deterministic. Fails if the rule is not applicable at the exact `target_span`.
*   **Patch Emission:** `SemanticPatch` with `PatchKind::Rewrite { rule_id, direction, target_span }`.
*   **Minimal Kernel Reliance:** `rule` primitive, elaborator's unification/matching engine, span-based AST manipulation.

---

### Tactic Category: Simplification

#### 3.9. `simp.once`

*   **Purpose:** Apply a set of configured rewrite rules (from `simp_cfg.maclane`) once, exhaustively, at the focused goal/hole.
*   **Input Expectations:** `TacticInput` with `target_hole_id` or `target_goal_id`.
*   **Determinism Policy:**
    1.  Uses a deterministically sorted list of rewrite rules from the global simplification configuration.
    2.  Applies rules in order, greedily.
    3.  Traversal order for sub-expressions is canonical (e.g., bottom-up, left-to-right).
    4.  The simplification process is exhaustive at the top-level once, i.e., it attempts to apply every rule at every possible location in the current focused term.
*   **Patch Emission:** A single `SemanticPatch` representing the cumulative effect of all rewrites, or a `BoundedList<SemanticPatch>` for individual steps if desired for fine-grained diffing. For simplicity, initially, a single patch describing the change from initial to final term.
*   **Minimal Kernel Reliance:** `rule` primitive, configuration for rewrite sets.

#### 3.10. `simp.fuel`

*   **Purpose:** Apply the simplification strategy (from `simp.once`) with an explicit fuel limit.
*   **Input Expectations:** `TacticInput` with `target_hole_id` or `target_goal_id`, `additional_args` containing `fuel: Int`.
*   **Determinism Policy:** Same as `simp.once`, but terminates deterministically after `fuel` rewrite applications or if no more rules are applicable.
*   **Patch Emission:** `SemanticPatch` (as in `simp.once`) or `TacticResult::Failure { reason: FuelExhausted }`.
*   **Minimal Kernel Reliance:** `rule` primitive, configuration for rewrite sets.

---

### Tactic Category: Case Analysis

#### 3.11. `cases.induction`

*   **Purpose:** Perform case analysis on an inductive hypothesis or definition within the current goal context, generating new subgoals for each case.
*   **Input Expectations:** `TacticInput` with `target_goal_id` or `target_hole_id`, `additional_args` containing `hypothesis_id: String` (identifier of the inductive hypothesis/def to case on).
*   **Determinism Policy:**
    1.  Identifies constructors of the inductive type deterministically.
    2.  Generates subgoals in a canonical, deterministic order (e.g., based on constructor declaration order).
    3.  Generates deterministic, fresh names for new bindings introduced in each case.
*   **Patch Emission:** `SemanticPatch` that replaces the original goal/hole with `begin`/`do` block containing new `def`s for each case, and new `Goal` objects in `new_subgoals`.
*   **Minimal Kernel Reliance:** `def`, `begin`/`do` primitives, knowledge of inductive types from elaborator.

---

### Tactic Category: Search / Automation

#### 3.12. `search.bfs`

*   **Purpose:** Perform a breadth-first search for a proof term, up to a specified depth or fuel limit.
*   **Input Expectations:** `TacticInput` with `target_goal_id` or `target_hole_id`, `additional_args` containing `depth_limit: Int` or `fuel: Int`.
*   **Determinism Policy:**
    1.  Explores the proof state graph layer by layer.
    2.  Applies candidate tactics/rules at each step in a deterministic order (e.g., sorted by tactic ID).
    3.  If multiple paths lead to the same goal, picks the canonically "first" one (e.g., lexicographically by path of applied tactics).
    4.  Terminates deterministically when a proof is found, depth/fuel limit is reached, or no more paths can be explored.
*   **Patch Emission:** `SemanticPatch` for the discovered proof term, or `TacticResult::Failure { reason: FuelExhausted }` or `NoProofFound`.
*   **Minimal Kernel Reliance:** Access to proof state, knowledge of available tactics/rules.

---

### Tactic Category: Utilities

#### 3.13. `utils.query_goal`

*   **Purpose:** Introspect and return structured information about the current focused goal or hole.
*   **Input Expectations:** `TacticInput` with `target_goal_id` or `target_hole_id`.
*   **Determinism Policy:** Deterministic. Always returns the same structured information for the same input.
*   **Patch Emission:** Emits an empty `SemanticPatch` (no change to document). The result is conveyed via `TacticResult::Success` with `ui_metadata` containing the structured goal info.
*   **Minimal Kernel Reliance:** Access to proof state.

---

This initial set provides a foundational set of tactics covering basic introduction, elimination (via rewrite/simp), and search. The emphasis is on building atomic, deterministic tactics that produce auditable `SemanticPatch`es, fully leveraging the `TACTICS_LAYER_SPEC.md`. Further tactics can be added following these principles.