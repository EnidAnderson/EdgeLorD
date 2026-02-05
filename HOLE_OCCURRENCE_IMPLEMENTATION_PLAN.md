# Hole Occurrence Implementation Plan

**Status:** v3 (final — for Sonnet implementation)
**Scope:** Complete implementation — kernel + EdgeLorD — nothing deferred

---

## 0. Orientation

EdgeLorD detects holes syntactically in `document.rs`. This works for basic
display but has three correctness problems:

1. **No authoritative enumeration.** EdgeLorD's syntactic walk and the
   elaborator's walk can disagree about what is a hole.
2. **Identity is derived from byte spans.** Goal IDs are
   `goal-{start}-{end}-{name}`, which breaks on any edit that shifts text.
3. **No ownership.** There is no record of which `def` a hole belongs to,
   so two holes named `?x` in different defs are indistinguishable.

This plan fixes all three by adding `HoleOccurrence` to the kernel's
elaboration output. The key design principle:

> **Spans are for UI positioning. `HoleId` is for debug correlation.**
> **Identity is `(owner, ordinal)`.**

`owner` is a `HoleOwner` enum that identifies the containing form:
`Def(name)` for holes inside a `def`, `Rule { rule_index }` for holes
inside a `rule`, or `TopLevel { form_index }` for holes outside any
def or rule. `ordinal` is a preorder counter of hole occurrences within
that owner, reset per owner. This tuple is deterministic, stable within
a compiled snapshot, and collision-free.

---

## 1. Glossary

| Term | Definition |
|------|-----------|
| **Hole** | A placeholder in a Mac Lane program. Written `?name` or `(hole name)` in surface syntax. Elaborates to `MorphismTerm::Hole(HoleId)`. |
| **HoleId** | A `u32` derived by FNV-hashing the hole name. **Not unique** — two holes named `?x` produce the same `HoleId`. Used only for kernel-term correlation, never as identity. |
| **HoleOccurrence** | New struct produced by the elaborator recording one hole: its owner, its ordinal within that owner, its span (for UI), its name (for display), and the binder names in scope. |
| **HoleOwner** | Enum identifying the form that owns a hole: `Def(String)` for named defs, `Rule { rule_index: u32 }` for rules (counted in source order), `TopLevel { form_index: u32 }` for top-level expressions outside any def/rule. |
| **ordinal** | A `u32` preorder counter of hole occurrences within a single owner, starting at 0 for each owner. |
| **Goal** | EdgeLorD's user-facing struct in `document.rs`. After this plan, its `goal_id` is derived from `(owner, ordinal)` when kernel data is available, falling back to `(span, name)` for syntactic-only mode. |
| **WorkspaceReport** | Returned by `ComradeWorkspace::did_open` / `did_change`. Gains a `holes: Vec<HoleOccurrence>` field. |
| **CoreBundleV0** | Output of the compilation pipeline. Gains a `holes: Vec<HoleOccurrence>` field. |

---

## 2. Architecture

```
  Source text
       |
       v
  [Parser]  ---- parse_module() ----> Module { body: Vec<SExpr> }
       |
       v
  [Expander] ---- expand_module() --> Vec<SExpr>  (macros resolved)
       |
       v
  [Elaborator] -- elaborate() ------> Vec<CoreForm>
       |                               + elaborator.holes: Vec<HoleOccurrence>     <-- NEW
       v
  [elaborate_query]  builds CoreBundleV0 { ..., holes: elaborator.holes }          <-- CHANGED
       |
       v
  [ComradeWorkspace::report_for_key]
       |    copies bundle.holes into WorkspaceReport.holes                         <-- CHANGED
       v
  [EdgeLorD ProofSession]
       |    stores holes per document in ProofDocument
       |    converts HoleOccurrence -> Goal using (owner, ordinal) identity
       v
  [EdgeLorD LSP Backend]
       |    hover, inlay hints, diagnostics all consume Goals
       |    syntactic fallback when elaboration fails
```

---

## 3. Layer K: Kernel Changes

All changes in `new_surface_syntax`. No changes to `tcb_core` or `codeswitch`.

### 3.1 Define types

**File:** `src/core.rs` — append after the existing `CompiledCoreBundle` type alias (line 107)

```rust
// ---------------------------------------------------------------------------
// Hole occurrence tracking (produced by the elaborator)
// ---------------------------------------------------------------------------

/// One occurrence of a hole found during elaboration.
///
/// Identity: `(owner, ordinal)`.
/// Span is for UI positioning only — never use it as a join key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoleOccurrence {
    /// FNV hash of the hole name. For kernel-term correlation only, NOT identity.
    pub hole_id: u32,

    /// User-visible name ("x" for `?x`, "h2" for `(hole h2)`).
    /// `None` for anonymous `(hole)` with no name argument.
    pub name: Option<String>,

    /// Byte span covering the entire hole form in source.
    /// For `?x`: span of the atom. For `(hole h2)`: span of the whole list.
    /// Used for LSP range conversion. NOT identity.
    pub span: Span,

    /// Surface syntax form used to write this hole.
    pub syntax: HoleSyntax,

    /// Which form owns this hole. This plus `ordinal` IS the identity.
    pub owner: HoleOwner,

    /// Preorder occurrence index of this hole within its owner.
    /// Reset to 0 for each new owner.
    pub ordinal: u32,

    /// Names of binders in scope at this hole, outermost first.
    /// Kind indicates whether the name was introduced by touch or def.
    /// (The elaborator's `Scope` does not track let-binders or binder spans;
    /// those are only available from EdgeLorD's syntactic fallback.)
    pub context: Vec<HoleContextEntry>,
}

/// Identifies the form that owns a hole occurrence.
/// Used as part of the identity tuple `(owner, ordinal)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoleOwner {
    /// Hole appears inside `(def name term)`.
    Def(String),
    /// Hole appears inside a `(rule ...)` form.
    /// `rule_index` is the 0-based source-order index among all rules.
    Rule { rule_index: u32 },
    /// Hole appears at top level, outside any def or rule.
    /// `form_index` is the 0-based source-order index among all top-level forms.
    TopLevel { form_index: u32 },
}

/// How the hole was written in surface syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleSyntax {
    /// `?name` — a prefixed atom.
    QuestionMark,
    /// `(hole name)` — an explicit hole form.
    HoleForm,
}

/// One entry in a hole's elaboration-level context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoleContextEntry {
    /// The binder name.
    pub name: String,
    /// How this name was introduced.
    pub kind: HoleBindingKind,
}

/// The kind of binding visible in a hole's elaboration context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleBindingKind {
    /// Introduced by `(touch name)`.
    Touch,
    /// Introduced by `(def name term)` — means both touched and defined.
    Def,
}
```

**Design notes:**
- `HoleContextEntry` has no `span` field because the elaborator's `Scope`
  does not track binder spans. This is honest — no `Span::new(0,0)` placeholders
  that would leak into UI or confuse tests.
- `HoleBindingKind` has no `Let` variant because the elaborator does not
  process `let` forms. Let-binder awareness exists only in EdgeLorD's
  syntactic layer (`document.rs`), which remains the fallback.
- `HoleOwner` + `ordinal` form a collision-free identity tuple without
  depending on spans or `HoleId` uniqueness. The enum variants prevent
  the collision that `Option<String>` with `None` would create when
  multiple rules or top-level forms each have holes at ordinal 0.

### 3.2 Add `holes` to `CoreBundleV0`

**File:** `src/core.rs` — in the `CoreBundleV0` struct (line 69-86)

Add after the `kernel_forms` field:

```rust
    /// Hole occurrences found during elaboration, in source order.
    /// Identity of each hole is `(owner, ordinal)`.
    pub holes: Vec<HoleOccurrence>,
```

Update `Default for CoreBundleV0` (line 88-100) to include:

```rust
            holes: Vec::new(),
```

**Construction site audit** — every `CoreBundleV0 { ... }` in the crate:

| File | Line | Fix |
|------|------|-----|
| `src/core.rs` | 88-100 (`Default`) | Add `holes: Vec::new()` |
| `src/lib.rs` | 156 (`elaborate_query`) | Add `holes: elaborator.holes` (see 3.3) |
| `src/mini_backend.rs` | 612, 628, 638, 677, 732, 741, 760, 764 | All use `..Default::default()` — automatic |

### 3.3 Modify the Elaborator to collect holes

**File:** `src/elaborate.rs`

**Step A: Add fields to `Elaborator`**

```rust
pub struct Elaborator {
    pub scope: Scope,
    pub macros: Vec<SugarForm>,
    pub rules: Vec<CompiledRule>,
    /// Collected hole occurrences, in source (preorder) order.
    pub holes: Vec<HoleOccurrence>,                                       // NEW
    /// The owner of the form currently being elaborated.
    current_owner: HoleOwner,                                             // NEW
    /// Per-owner ordinal counter, reset each time owner changes.
    current_ordinal: u32,                                                 // NEW
    /// Counter for rules seen so far (for HoleOwner::Rule index).
    rule_count: u32,                                                      // NEW
    /// Counter for top-level forms seen so far (for HoleOwner::TopLevel index).
    form_count: u32,                                                      // NEW
}
```

Update `Elaborator::new()`:

```rust
    pub fn new() -> Self {
        Self {
            scope: Scope::default(),
            macros: Vec::new(),
            rules: Vec::new(),
            holes: Vec::new(),
            current_owner: HoleOwner::TopLevel { form_index: 0 },
            current_ordinal: 0,
            rule_count: 0,
            form_count: 0,
        }
    }
```

**Step B: Track current_owner around `def`, `rule`, and top-level elaboration**

In the outer `elaborate()` method (the loop that iterates over top-level
forms and calls `elaborate_expr` for each), set the owner before each form:

```rust
// In elaborate() — the method that processes the Vec<SExpr> from expansion:
for form in &expanded_forms {
    // Default to TopLevel owner for each top-level form.
    self.current_owner = HoleOwner::TopLevel { form_index: self.form_count };
    self.current_ordinal = 0;
    self.form_count += 1;

    self.elaborate_expr(form)?;
}
```

In `elaborate_expr`, in the `"def"` arm, wrap the `sexpr_to_morphism` call
so the elaborator knows which def it's inside:

```rust
"def" => {
    // ... existing arity checks and scope checks ...
    // (lines 84-120 stay the same)

    // Set current_owner so hole collection knows the owner.
    let prev_owner = std::mem::replace(
        &mut self.current_owner,
        HoleOwner::Def(name.clone()),
    );
    let prev_ord = self.current_ordinal;
    self.current_ordinal = 0;

    let term = self.sexpr_to_morphism_collecting(&elements[2])?;

    // Restore previous owner context (for nested forms, though
    // the current elaborator doesn't support nested defs).
    self.current_owner = prev_owner;
    self.current_ordinal = prev_ord;

    self.scope
        .defs
        .insert(name.clone(), CoreForm::Def(name.clone(), term.clone()));
    Ok(CoreForm::Def(name, term))
}
```

Similarly, in the `"rule"` arm, set the owner to `Rule` and advance the
rule counter:

```rust
"rule" => {
    // ... existing arity check ...

    let prev_owner = std::mem::replace(
        &mut self.current_owner,
        HoleOwner::Rule { rule_index: self.rule_count },
    );
    self.rule_count += 1;
    let prev_ord = self.current_ordinal;
    self.current_ordinal = 0;

    let lhs = self.sexpr_to_morphism_collecting(&elements[1])?;
    let rhs = self.sexpr_to_morphism_collecting(&elements[2])?;

    self.current_owner = prev_owner;
    self.current_ordinal = prev_ord;

    let meta = Self::parse_meta(&elements[3])?;
    let compiled = Self::normalize_rule(lhs, rhs, meta, expr.span)?;
    self.rules.push(compiled.clone());
    Ok(CoreForm::Rule(compiled.lhs, compiled.rhs, compiled.meta))
}
```

**Step C: Create instance method `sexpr_to_morphism_collecting`**

This is an instance method that wraps the existing static
`sexpr_to_hyperedge`, but additionally records `HoleOccurrence`s.

**CRITICAL: Bare symbol handling.** In the existing static
`sexpr_to_hyperedge`, bare (non-`?`) symbols like `y` in `(def a y)` are
turned into `MorphismTerm::Hole(hole_name_to_id(sym))` at line 249. This
is a kernel placeholder, NOT a user-written hole. The new method must
handle bare symbols in its own arm to produce the same `MorphismTerm` for
kernel semantics but NOT call `record_hole()`. Only `?name` atoms and
`(hole ...)` forms are real holes and get recorded.

```rust
    /// Like `sexpr_to_hyperedge`, but also records hole occurrences using
    /// the elaborator's current owner context and scope.
    ///
    /// Bare symbols (non-`?` atoms) still produce `MorphismTerm::Hole` for
    /// kernel-level placeholder semantics, but are NOT recorded as
    /// `HoleOccurrence`s. Only explicit `?name` and `(hole name)` forms
    /// are user-written holes.
    fn sexpr_to_morphism_collecting(
        &mut self,
        expr: &SExpr,
    ) -> Result<MorphismTerm, ElaborationError> {
        match &expr.kind {
            // User-written hole: `?name` — record as HoleOccurrence.
            SExprKind::Atom(Atom::Symbol(sym)) if sym.starts_with('?') => {
                let hole_name = sym.trim_start_matches('?');
                if hole_name.is_empty() {
                    return Ok(MorphismTerm::Reject {
                        code: "invalid-hole".into(),
                        msg: format!("Empty hole name in {:?}", sym),
                    });
                }
                let hole_id = hole_name_to_id(hole_name);
                self.record_hole(
                    hole_id,
                    Some(hole_name.to_string()),
                    expr.span,
                    HoleSyntax::QuestionMark,
                );
                Ok(MorphismTerm::Hole(hole_id))
            }

            // Bare symbol (non-`?`): kernel placeholder, NOT a user hole.
            // Produces the same MorphismTerm::Hole as sexpr_to_hyperedge
            // line 249, but does NOT call record_hole().
            SExprKind::Atom(Atom::Symbol(sym)) => {
                Ok(MorphismTerm::Hole(hole_name_to_id(sym)))
            }

            // User-written hole: `(hole name)` — record as HoleOccurrence.
            SExprKind::List(items) if is_hole_form(items) => {
                let hole_name = hole_form_name(items);
                let hole_id = hole_name_to_id(
                    hole_name.as_deref().unwrap_or("anonymous"),
                );
                self.record_hole(
                    hole_id,
                    hole_name,
                    expr.span,
                    HoleSyntax::HoleForm,
                );
                Ok(MorphismTerm::Hole(hole_id))
            }

            SExprKind::List(items) => {
                // Recurse into list elements
                if items.is_empty() {
                    return Ok(MorphismTerm::Reject {
                        code: "empty-application".into(),
                        msg: "Empty list not allowed in kernel".into(),
                    });
                }
                let mut components = Vec::new();
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();
                for item in items {
                    let term = self.sexpr_to_morphism_collecting(item)?;
                    if let Some((gen_inputs, gen_outputs)) = Self::term_io_vectors(&term) {
                        inputs.extend(gen_inputs);
                        outputs.extend(gen_outputs);
                    }
                    components.push(term);
                }
                Ok(MorphismTerm::Compose {
                    components,
                    inputs,
                    outputs,
                    doctrine: None,
                })
            }

            // Quotes, integers, string literals: delegate to existing static.
            // These cannot contain holes.
            _ => Self::sexpr_to_hyperedge(expr),
        }
    }

    /// Record a hole occurrence using the current elaboration context.
    fn record_hole(
        &mut self,
        hole_id: u32,
        name: Option<String>,
        span: Span,
        syntax: HoleSyntax,
    ) {
        let context = self.snapshot_context();
        let ordinal = self.current_ordinal;
        self.current_ordinal += 1;
        self.holes.push(HoleOccurrence {
            hole_id,
            name,
            span,
            syntax,
            owner: self.current_owner.clone(),
            ordinal,
            context,
        });
    }

    /// Snapshot the current elaboration scope as hole context entries.
    fn snapshot_context(&self) -> Vec<HoleContextEntry> {
        self.scope
            .binders
            .iter()
            .map(|name| HoleContextEntry {
                name: name.clone(),
                kind: if self.scope.defs.contains_key(name) {
                    HoleBindingKind::Def
                } else {
                    HoleBindingKind::Touch
                },
            })
            .collect()
    }
```

**Step D: Add helper functions** (module-level in `elaborate.rs`)

```rust
/// FNV-1a hash of a hole name to a u32. Matches the existing inline helper.
fn hole_name_to_id(name: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(16777619);
        hash ^= byte as u32;
    }
    hash
}

/// Check if a list is a `(hole ...)` form.
fn is_hole_form(items: &[SExpr]) -> bool {
    items.len() >= 1
        && matches!(
            &items[0].kind,
            SExprKind::Atom(Atom::Symbol(s)) if s == "hole"
        )
}

/// Extract the name from a `(hole name)` form.
fn hole_form_name(items: &[SExpr]) -> Option<String> {
    if items.len() < 2 {
        return None;
    }
    match &items[1].kind {
        SExprKind::Atom(Atom::Symbol(name)) => Some(name.clone()),
        _ => None,
    }
}
```

**Step E: Use the new type in imports**

At the top of `elaborate.rs`, add:

```rust
use super::core::{HoleOccurrence, HoleOwner, HoleSyntax, HoleBindingKind, HoleContextEntry};
```

### 3.4 Wire holes into `elaborate_query`

**File:** `src/lib.rs`, in `elaborate_query()` (line 142-165)

Change the `CoreBundleV0` construction at line 156-164:

```rust
    Ok(CoreBundleV0 {
        forms: core_forms,
        macros: expanded.macros,
        rules: elaborator.rules,
        imports: crate::ImportEnv::new(),
        import_declarations: expanded.import_declarations,
        ambient_doctrine: expanded.ambient_doctrine,
        kernel_forms,
        holes: elaborator.holes,                                           // NEW
    })
```

### 3.5 Add `holes` to `WorkspaceReport`

**File:** `src/comrade_workspace.rs`

Add to the `WorkspaceReport` struct (after line 24):

```rust
    /// Hole occurrences from the most recent elaboration.
    /// Empty if elaboration failed or there are no holes.
    pub holes: Vec<super::core::HoleOccurrence>,
```

Update the two construction sites in `report_for_key` (lines 176-188):

Success path (line 176):
```rust
            Ok(bundle) => {
                let holes = bundle.holes.clone();
                Ok(WorkspaceReport {
                    diagnostics: Vec::new(),
                    fingerprint: Self::fingerprint_for_bundle(&bundle),
                    revision,
                    bundle: Some(bundle),
                    holes,
                })
            }
```

Error path (line 182):
```rust
            Err(err) => Ok(WorkspaceReport {
                diagnostics: vec![workspace_diagnostic_from_surface_error(&err)],
                fingerprint: None,
                revision,
                bundle: None,
                holes: Vec::new(),
            }),
```

### 3.6 Export new types

**File:** `src/lib.rs`, in the `pub use core::{...}` block (line 31-33)

Change:
```rust
pub use core::{
    CompiledCoreBundle, CompiledRule, CoreBundleV0, CoreForm, ImportEnv, Meta, SugarForm,
};
```
To:
```rust
pub use core::{
    CompiledCoreBundle, CompiledRule, CoreBundleV0, CoreForm, HoleBindingKind,
    HoleContextEntry, HoleOccurrence, HoleOwner, HoleSyntax, ImportEnv, Meta, SugarForm,
};
```

### 3.7 Tests (kernel)

Add to `src/lib.rs` `#[cfg(test)] mod tests` (or a new test file):

```rust
#[test]
fn hole_occurrences_from_question_mark_syntax() {
    let source = "(touch a)\n(def a ?x)";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 1);
    let h = &bundle.holes[0];
    assert_eq!(h.name.as_deref(), Some("x"));
    assert_eq!(h.syntax, HoleSyntax::QuestionMark);
    assert_eq!(h.owner, HoleOwner::Def("a".to_string()));
    assert_eq!(h.ordinal, 0);
    assert!(h.context.iter().any(|c| c.name == "a"));
}

#[test]
fn hole_occurrences_from_hole_form_syntax() {
    let source = "(touch b)\n(def b (hole h))";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 1);
    let h = &bundle.holes[0];
    assert_eq!(h.name.as_deref(), Some("h"));
    assert_eq!(h.syntax, HoleSyntax::HoleForm);
    assert_eq!(h.owner, HoleOwner::Def("b".to_string()));
    assert_eq!(h.ordinal, 0);
}

#[test]
fn hole_context_reflects_elaboration_scope() {
    let source = "(touch x)\n(touch y)\n(def x someterm)\n(def y ?goal)";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 1);
    let h = &bundle.holes[0];
    // x is Def (touched and defined), y is Touch (touched, being defined now)
    let x_entry = h.context.iter().find(|c| c.name == "x");
    let y_entry = h.context.iter().find(|c| c.name == "y");
    assert!(x_entry.is_some());
    assert!(y_entry.is_some());
    assert_eq!(x_entry.unwrap().kind, HoleBindingKind::Def);
    assert_eq!(y_entry.unwrap().kind, HoleBindingKind::Touch);
}

#[test]
fn ordinal_resets_per_def() {
    // Two defs, each with one hole → both ordinals are 0.
    let source = "(touch a)\n(def a ?x)\n(touch b)\n(def b ?y)";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 2);
    assert_eq!(bundle.holes[0].owner, HoleOwner::Def("a".to_string()));
    assert_eq!(bundle.holes[0].ordinal, 0);
    assert_eq!(bundle.holes[1].owner, HoleOwner::Def("b".to_string()));
    assert_eq!(bundle.holes[1].ordinal, 0);
}

#[test]
fn ordinal_is_preorder_within_def() {
    // One def with a nested term containing two holes.
    let source = "(touch f)\n(def f (?x ?y))";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 2);
    assert_eq!(bundle.holes[0].owner, HoleOwner::Def("f".to_string()));
    assert_eq!(bundle.holes[0].ordinal, 0);
    assert_eq!(bundle.holes[0].name.as_deref(), Some("x"));
    assert_eq!(bundle.holes[1].owner, HoleOwner::Def("f".to_string()));
    assert_eq!(bundle.holes[1].ordinal, 1);
    assert_eq!(bundle.holes[1].name.as_deref(), Some("y"));
}

#[test]
fn duplicate_hole_names_get_distinct_ordinals() {
    // Same name ?x twice in one def → same HoleId but different ordinals.
    let source = "(touch f)\n(def f (?x ?x))";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 2);
    assert_eq!(bundle.holes[0].hole_id, bundle.holes[1].hole_id); // same name → same hash
    assert_eq!(bundle.holes[0].ordinal, 0);
    assert_eq!(bundle.holes[1].ordinal, 1);
    // Identity is (owner, ordinal), not hole_id
    assert_ne!(
        (&bundle.holes[0].owner, bundle.holes[0].ordinal),
        (&bundle.holes[1].owner, bundle.holes[1].ordinal)
    );
}

#[test]
fn rule_holes_get_rule_owner() {
    // Holes inside rules get HoleOwner::Rule with correct index.
    let source = "(touch a)\n(def a someterm)\n(rule ?lhs ?rhs (meta prov))";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    let rule_holes: Vec<_> = bundle.holes.iter()
        .filter(|h| matches!(h.owner, HoleOwner::Rule { .. }))
        .collect();
    assert_eq!(rule_holes.len(), 2); // ?lhs and ?rhs
    assert_eq!(rule_holes[0].owner, HoleOwner::Rule { rule_index: 0 });
    assert_eq!(rule_holes[0].ordinal, 0);
    assert_eq!(rule_holes[1].owner, HoleOwner::Rule { rule_index: 0 });
    assert_eq!(rule_holes[1].ordinal, 1);
}

#[test]
fn bare_symbols_are_not_recorded_as_holes() {
    // `y` in `(def a y)` becomes MorphismTerm::Hole for kernel semantics
    // but must NOT appear in the hole occurrence list.
    let source = "(touch a)\n(def a y)";
    let bundle = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(bundle.holes.len(), 0, "bare symbols must not be recorded as holes");
}

#[test]
fn holes_survive_workspace_report() {
    let mut ws = ComradeWorkspace::new();
    let report = ws.did_open("test", "(touch a)\n(def a ?x)").unwrap();
    assert_eq!(report.holes.len(), 1);
    assert_eq!(report.holes[0].name.as_deref(), Some("x"));
}

#[test]
fn no_holes_when_elaboration_fails() {
    let mut ws = ComradeWorkspace::new();
    let report = ws.did_open("test", "(def x ?h)").unwrap(); // no touch → elab error
    assert!(report.holes.is_empty());
}

#[test]
fn hole_occurrences_are_deterministic() {
    let source = "(touch a)\n(def a ?x)\n(touch b)\n(def b ?y)";
    let a = compile_surface(source, &CompileOptions::default()).unwrap();
    let b = compile_surface(source, &CompileOptions::default()).unwrap();
    assert_eq!(a.holes, b.holes);
}
```

---

## 4. Layer E: EdgeLorD Changes

### 4.1 Update `WorkspaceReport` construction sites in EdgeLorD

EdgeLorD constructs `WorkspaceReport` directly in several places.
Each needs the new `holes` field.

**File: `src/lsp.rs`**

| Location | Code | Fix |
|----------|------|-----|
| `workspace_error_report()` (line 347-354) | `WorkspaceReport { diagnostics, fingerprint: None, revision: 0, bundle: None }` | Add `holes: Vec::new()` |
| `sample_report()` test helper (line 875-895) | `WorkspaceReport { diagnostics, fingerprint: None, revision: 0, bundle: None }` | Add `holes: Vec::new()` |
| test at line 908 | `WorkspaceReport { diagnostics, ... }` | Add `holes: Vec::new()` |

**File: `src/proof_session.rs`**

| Location | Code | Fix |
|----------|------|-----|
| `update()` error path (line 84) | `WorkspaceReport { diagnostics: Vec::new(), ... }` | Add `holes: Vec::new()` |
| `apply_command()` error path (line 145) | `WorkspaceReport { diagnostics: Vec::new(), ... }` | Add `holes: Vec::new()` |

### 4.2 Store holes in ProofDocument

**File:** `src/proof_session.rs`

Add a field to `ProofDocument`:

```rust
pub struct ProofDocument {
    pub version: i32,
    pub parsed: ParsedDocument,
    pub last_analyzed: Instant,
    pub workspace_report: WorkspaceReport,
    pub holes: Vec<new_surface_syntax::HoleOccurrence>,                    // NEW
}
```

In `ProofSession::open()`, when inserting the document (line 65-70):

```rust
        let holes = report.holes.clone();
        self.documents.insert(uri, ProofDocument {
            version,
            parsed,
            last_analyzed: Instant::now(),
            workspace_report: report.clone(),
            holes,
        });
```

Same pattern in `ProofSession::update()` (line 119-124):

```rust
        let holes = report.holes.clone();
        self.documents.insert(uri, ProofDocument {
            version,
            parsed,
            last_analyzed: Instant::now(),
            workspace_report: report.clone(),
            holes,
        });
```

### 4.3 Convert HoleOccurrence to Goal with correct identity

**File:** `src/document.rs`

**Step A: Change `Binding.span` from `ByteSpan` to `Option<ByteSpan>`**

The elaborator does not track binder spans (its `Scope` is just names).
Using `ByteSpan::new(0, 0)` would violate the no-fake-spans principle.
Instead, make the span optional:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub name: String,
    pub kind: BindingKind,
    pub span: Option<ByteSpan>,       // CHANGED from ByteSpan
    pub value_preview: Option<String>,
    pub ty_preview: Option<String>,
}
```

Update all existing callers that construct `Binding` with a concrete span
to wrap it in `Some(...)`:
- `collect_top_level_bindings()` → `span: Some(ByteSpan::new(...))`
- `let_bindings()` → `span: Some(ByteSpan::new(...))`
- Any test code constructing `Binding` — wrap spans in `Some()`

Update all callers that read `Binding.span`:
- `format_context()` in `src/lsp.rs` — use `.span.map(|s| ...).unwrap_or_default()` or skip span display when `None`
- `merged_context()` in `src/document.rs` — use `.span.unwrap_or(ByteSpan::new(0,0))` only for sorting/dedup, or sort by name when span is `None`

**Step B: Add conversion function**

```rust
/// Convert kernel-produced hole occurrences to EdgeLorD goals.
///
/// Identity is `(owner, ordinal)` — NOT span, NOT HoleId.
/// Span is used only for LSP range positioning.
pub fn goals_from_hole_occurrences(
    holes: &[new_surface_syntax::HoleOccurrence],
) -> Vec<Goal> {
    holes.iter().map(|h| {
        let owner_str = match &h.owner {
            new_surface_syntax::HoleOwner::Def(name) => name.clone(),
            new_surface_syntax::HoleOwner::Rule { rule_index } =>
                format!("_rule{}", rule_index),
            new_surface_syntax::HoleOwner::TopLevel { form_index } =>
                format!("_top{}", form_index),
        };
        let goal_id = format!("goal:{}:{}", owner_str, h.ordinal);
        let name = h.name.clone();
        Goal {
            goal_id,
            name,
            span: ByteSpan::new(h.span.start, h.span.end),
            context: h.context.iter().map(|entry| Binding {
                name: entry.name.clone(),
                kind: match entry.kind {
                    new_surface_syntax::HoleBindingKind::Touch => BindingKind::Touch,
                    new_surface_syntax::HoleBindingKind::Def => BindingKind::Def,
                },
                span: None, // elaborator does not track binder spans — honest None
                value_preview: None,
                ty_preview: None,
            }).collect(),
            target: "unknown".to_string(),
        }
    }).collect()
}
```

### 4.4 Hybrid goal strategy

**File:** `src/proof_session.rs`

In both `open()` and `update()`, replace the current goal extraction with:

```rust
use crate::document::goals_from_hole_occurrences;

// After getting `report` and `parsed`:
let goals = if !report.holes.is_empty() {
    // Kernel elaboration succeeded and found holes — authoritative.
    goals_from_hole_occurrences(&report.holes)
} else {
    // Elaboration failed or no holes found — fall back to syntactic detection.
    // This covers parse-error states where the user still wants to see goals.
    parsed.goals.clone()
};
```

Use this `goals` value in the result structs.

### 4.5 Generate hole diagnostics

**File:** `src/lsp.rs`

Add function:

```rust
/// Generate informational diagnostics for unsolved holes.
fn hole_diagnostics(
    holes: &[new_surface_syntax::HoleOccurrence],
    text: &str,
) -> Vec<Diagnostic> {
    holes.iter().map(|h| {
        let name = h.name.as_deref().unwrap_or("?");
        let owner = match &h.owner {
            new_surface_syntax::HoleOwner::Def(name) => format!("def `{}`", name),
            new_surface_syntax::HoleOwner::Rule { rule_index } =>
                format!("rule #{}", rule_index),
            new_surface_syntax::HoleOwner::TopLevel { form_index } =>
                format!("top-level form #{}", form_index),
        };
        Diagnostic {
            range: byte_span_to_range(text, ByteSpan::new(h.span.start, h.span.end)),
            severity: Some(DiagnosticSeverity::INFORMATION),
            code: Some(NumberOrString::String("unsolved-goal".to_string())),
            code_description: None,
            source: Some("edgelord-lsp".to_string()),
            message: format!("unsolved goal `{}` in {}", name, owner),
            related_information: None,
            tags: None,
            data: None,
        }
    }).collect()
}
```

Update `document_diagnostics_from_report` to accept holes and merge:

```rust
pub fn document_diagnostics_from_report(
    uri: &Url,
    report: &WorkspaceReport,
    parsed_doc: &ParsedDocument,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(parsed_doc.diagnostics.iter().map(|pd| /* ... existing ... */));
    diagnostics.extend(workspace_report_to_diagnostics(report, &parsed_doc.text));
    diagnostics.extend(hole_diagnostics(&report.holes, &parsed_doc.text));     // NEW
    sort_diagnostics(uri, &mut diagnostics);
    diagnostics
}
```

### 4.6 Suppress "target: unknown" in hover

**File:** `src/lsp.rs`, in `Backend::hover()` (around line 678-683)

Replace the goal display block:

```rust
if let Some(goal) = doc.goal_at_offset(offset) {
    let goal_name = goal.name.as_deref().unwrap_or("?");
    let ctx = format_context(&goal.context, 8);
    content.push_str(&format!(
        "**Goal** `{}`\n\n- id: `{}`\n- context: {}",
        goal_name, goal.goal_id, ctx
    ));
    // Only show target if it carries real information
    if goal.target != "unknown" {
        content.push_str(&format!("\n- target: `{}`", goal.target));
    }
}
```

### 4.7 Tests (EdgeLorD)

Add to existing test files or a new `tests/mvp1_3_hole_occurrences.rs`:

```rust
use edgelord_lsp::document::{
    goals_from_hole_occurrences, ByteSpan, Goal, BindingKind, ParsedDocument,
};
use new_surface_syntax::{
    HoleOccurrence, HoleOwner, HoleSyntax, HoleContextEntry, HoleBindingKind,
};
use source_span::Span;

fn make_hole(
    name: &str,
    owner: &str,
    ordinal: u32,
    start: usize,
    end: usize,
) -> HoleOccurrence {
    HoleOccurrence {
        hole_id: 0, // irrelevant for identity
        name: Some(name.to_string()),
        span: Span::new(start, end),
        syntax: HoleSyntax::QuestionMark,
        owner: HoleOwner::Def(owner.to_string()),
        ordinal,
        context: vec![HoleContextEntry {
            name: owner.to_string(),
            kind: HoleBindingKind::Touch,
        }],
    }
}

#[test]
fn conversion_uses_owner_ordinal_identity() {
    let holes = vec![
        make_hole("x", "a", 0, 10, 12),
        make_hole("y", "b", 0, 20, 22),
    ];
    let goals = goals_from_hole_occurrences(&holes);
    assert_eq!(goals.len(), 2);
    assert_eq!(goals[0].goal_id, "goal:a:0");
    assert_eq!(goals[1].goal_id, "goal:b:0");
}

#[test]
fn duplicate_names_get_distinct_goal_ids() {
    let holes = vec![
        make_hole("x", "f", 0, 10, 12),
        make_hole("x", "f", 1, 14, 16),
    ];
    let goals = goals_from_hole_occurrences(&holes);
    assert_eq!(goals.len(), 2);
    assert_ne!(goals[0].goal_id, goals[1].goal_id);
    assert_eq!(goals[0].goal_id, "goal:f:0");
    assert_eq!(goals[1].goal_id, "goal:f:1");
}

#[test]
fn hybrid_fallback_uses_syntactic_when_no_kernel_holes() {
    // Simulate: report.holes is empty (elaboration failed)
    // ParsedDocument still finds holes syntactically
    let text = "(def p ?h)\n";
    let parsed = ParsedDocument::parse(text.to_string());
    assert!(!parsed.goals.is_empty(), "syntactic detection must find the hole");

    let kernel_holes: Vec<HoleOccurrence> = Vec::new();
    let goals = if !kernel_holes.is_empty() {
        goals_from_hole_occurrences(&kernel_holes)
    } else {
        parsed.goals.clone()
    };
    assert!(!goals.is_empty(), "fallback must preserve syntactic goals");
    assert!(goals[0].goal_id.starts_with("goal-"), "syntactic IDs use old format");
}

#[test]
fn context_conversion_maps_kinds_correctly() {
    let holes = vec![HoleOccurrence {
        hole_id: 0,
        name: Some("g".to_string()),
        span: Span::new(0, 2),
        syntax: HoleSyntax::QuestionMark,
        owner: HoleOwner::Def("f".to_string()),
        ordinal: 0,
        context: vec![
            HoleContextEntry {
                name: "x".to_string(),
                kind: HoleBindingKind::Touch,
            },
            HoleContextEntry {
                name: "y".to_string(),
                kind: HoleBindingKind::Def,
            },
        ],
    }];
    let goals = goals_from_hole_occurrences(&holes);
    assert_eq!(goals[0].context.len(), 2);
    assert_eq!(goals[0].context[0].kind, BindingKind::Touch);
    assert_eq!(goals[0].context[1].kind, BindingKind::Def);
    // Kernel-derived bindings have no span (elaborator doesn't track binder spans)
    assert_eq!(goals[0].context[0].span, None);
    assert_eq!(goals[0].context[1].span, None);
}
```

---

## 5. Implementation Order (Gate-Aware)

Each gate must pass before proceeding to the next. Each is a single commit.

### Gate 1: Types compile (Kernel)
**What:** Add `HoleOccurrence` and related types to `core.rs`. Add `holes`
field to `CoreBundleV0` and `WorkspaceReport`. Update `Default` impls and
all construction sites to `Vec::new()`. Export types from `lib.rs`.

**Gate test:** `cargo check -p new_surface_syntax` succeeds.
All existing tests pass: `cargo test -p new_surface_syntax`.

### Gate 2: Elaborator collects holes (Kernel)
**What:** Add `holes`, `current_owner`, `current_ordinal`, `rule_count`,
`form_count` fields to `Elaborator`. Add `sexpr_to_morphism_collecting`,
`record_hole`, `snapshot_context` instance methods. Add `hole_name_to_id`,
`is_hole_form`, `hole_form_name` module-level helpers. Wire into
`elaborate()` top-level loop and `elaborate_expr` for `def` and `rule`
arms. Wire `elaborator.holes` into `CoreBundleV0` construction in
`elaborate_query`.

**Gate test:** All 12 kernel tests from Section 3.7 pass.
All existing tests still pass.

### Gate 3: EdgeLorD consumes holes (EdgeLorD)
**What:** Update all `WorkspaceReport` construction sites in EdgeLorD to
include `holes: Vec::new()`. Add `holes` to `ProofDocument`. Change
`Binding.span` from `ByteSpan` to `Option<ByteSpan>` and update all
callers. Add `goals_from_hole_occurrences()`. Implement hybrid strategy.
Add `hole_diagnostics()`. Update hover. Add import of new types.

**Gate test:** All 4 EdgeLorD tests from Section 4.7 pass.
All existing EdgeLorD tests pass: `cargo test -p edgelord-lsp`.

### Gate 4: Integration
**What:** Open a `.comrade` file in Helix containing `?x` holes. Verify:
- Inlay hints appear (same as before, but now driven by kernel data when
  elaboration succeeds).
- Hover shows goal info without "target: unknown" clutter.
- Diagnostics include "unsolved goal" informational entries.
- Files with parse errors still show goals (syntactic fallback).

**Gate test:** Manual verification in Helix. Confirm no regressions.

---

## 6. Files Modified (Complete Audit)

### Kernel (`new_surface_syntax`)

| File | Lines | Change |
|------|-------|--------|
| `src/core.rs` | After line 107 | Add `HoleOccurrence`, `HoleSyntax`, `HoleContextEntry`, `HoleBindingKind` |
| `src/core.rs` | Line 85 (in struct) | Add `holes: Vec<HoleOccurrence>` to `CoreBundleV0` |
| `src/core.rs` | Line 97 (in Default) | Add `holes: Vec::new()` |
| `src/elaborate.rs` | Line 13-19 (struct) | Add `holes`, `current_owner`, `current_ordinal`, `rule_count`, `form_count` to `Elaborator` |
| `src/elaborate.rs` | Line 32-38 (new) | Update `Elaborator::new()` |
| `src/elaborate.rs` | `elaborate()` loop | Set `current_owner = TopLevel` and advance `form_count` before each form |
| `src/elaborate.rs` | Line 84-126 (def arm) | Wrap with `current_owner`/`current_ordinal` management, use `sexpr_to_morphism_collecting` |
| `src/elaborate.rs` | Line 127-141 (rule arm) | Same wrapping pattern with `HoleOwner::Rule`, advance `rule_count` |
| `src/elaborate.rs` | New methods | `sexpr_to_morphism_collecting`, `record_hole`, `snapshot_context` |
| `src/elaborate.rs` | New helpers | `hole_name_to_id`, `is_hole_form`, `hole_form_name` |
| `src/comrade_workspace.rs` | Line 24 (in struct) | Add `holes: Vec<HoleOccurrence>` to `WorkspaceReport` |
| `src/comrade_workspace.rs` | Lines 176-188 | Add `holes` to both construction sites in `report_for_key` |
| `src/lib.rs` | Line 31-33 | Add new types to `pub use core::` |
| `src/lib.rs` | Line 156-164 | Add `holes: elaborator.holes` to `CoreBundleV0` construction |
| Tests | New | 12 tests in Section 3.7 |

### EdgeLorD

| File | Lines | Change |
|------|-------|--------|
| `src/proof_session.rs` | Line 17-22 (struct) | Add `holes` to `ProofDocument` |
| `src/proof_session.rs` | Lines 53-77 (open) | Store `report.holes`, use hybrid goal strategy |
| `src/proof_session.rs` | Lines 79-131 (update) | Same |
| `src/proof_session.rs` | Lines 84, 145 | Add `holes: Vec::new()` to error-path `WorkspaceReport` |
| `src/document.rs` | Line 70 (struct) | Change `Binding.span` from `ByteSpan` to `Option<ByteSpan>` |
| `src/document.rs` | Various | Wrap existing `Binding` span constructions in `Some()` |
| `src/document.rs` | New function | `goals_from_hole_occurrences()` |
| `src/lsp.rs` | Lines 347-354 | Add `holes: Vec::new()` to `workspace_error_report` |
| `src/lsp.rs` | New function | `hole_diagnostics()` |
| `src/lsp.rs` | Lines 205-228 | Update `document_diagnostics_from_report` to include hole diagnostics |
| `src/lsp.rs` | `format_context()` | Handle `Binding.span: Option<ByteSpan>` (skip or default when `None`) |
| `src/lsp.rs` | Lines 678-683 | Suppress "target: unknown" in hover |
| `src/lsp.rs` | Lines 875, 908 | Add `holes: Vec::new()` to test helpers |
| Tests | New | 4 tests in Section 4.7 |

---

## 7. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| `sexpr_to_hyperedge` is static; new `sexpr_to_morphism_collecting` duplicates traversal logic | Code duplication in `elaborate.rs` | The new method explicitly handles all atom cases (including bare symbols) to avoid accidentally recording non-hole symbols. It delegates only quotes/literals to the static method. Keep the static method as-is for backward compatibility (rule normalization, tests). |
| Bare symbols produce `MorphismTerm::Hole` | Could pollute hole list | Handled: `sexpr_to_morphism_collecting` has an explicit arm for non-`?` symbols that produces the kernel placeholder `MorphismTerm::Hole(hole_name_to_id(sym))` without calling `record_hole()`. Test `bare_symbols_are_not_recorded_as_holes` enforces this. |
| `CoreBundleV0` gains a field; all construction sites break | Compile errors (not logic errors) | All `mini_backend.rs` sites use `..Default::default()` (automatic). Only `lib.rs:156` needs an explicit addition. |
| `Binding.span` changes from `ByteSpan` to `Option<ByteSpan>` | All existing code constructing or reading `Binding.span` breaks | Compile errors guide fixes. Wrap existing spans in `Some()`. Kernel-derived bindings use `None` (honest). |
| Elaborator doesn't handle `let` binders | Kernel context is less complete than syntactic context | Document honestly. The syntactic fallback in EdgeLorD still provides let-binder context when elaboration fails. When elaboration succeeds, the kernel context has the authoritative touch/def scope. |

---

## 8. Success Criteria

1. `cargo test -p new_surface_syntax` — all existing + 12 new tests pass.
2. `cargo test -p edgelord-lsp` — all existing + 4 new tests pass.
3. In Helix: `.comrade` file with holes shows inlay hints and hover.
4. Goal IDs in hover show `goal:defname:N` format (kernel-sourced).
5. Files with parse errors still show goals via syntactic fallback
   (IDs show `goal-start-end-name` format).
6. "unsolved goal" informational diagnostics appear for each hole.
7. No `Span::new(0,0)` or `ByteSpan::new(0,0)` values appear in any user-visible output.
   Kernel-derived `Binding` entries use `span: None`.
8. Duplicate-named holes (`?x` twice in one def) produce distinct goal IDs.
9. Bare symbols like `y` in `(def a y)` do NOT appear in the hole list.
10. Holes in different rules get distinct `HoleOwner::Rule { rule_index }` owners.
