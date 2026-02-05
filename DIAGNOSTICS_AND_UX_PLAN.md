# World-Class Diagnostics, Pretty-Printing, and UX Plan

**Status:** v1 (for review)
**Scope:** Complete — kernel + EdgeLorD — nothing deferred

---

## 1. Executive Summary

Mac Lane's diagnostics are functional but not yet excellent. Error messages are
string-formatted, spans are sometimes missing or fabricated, there is no
pretty-printing for complex terms, no lint framework, no "explain" features,
and no structured diagnostic payloads for rich IDE integration.

This plan upgrades the entire diagnostic pipeline to world-class:

1. **Structured diagnostics** with codes, labeled spans, explanations, hints,
   and quick fixes — carried end-to-end from kernel to LSP.
2. **Pretty-printers** for every term type (SExpr, CoreForm, MorphismTerm,
   rules, scopes, contexts, rewrite traces) with compact and explanatory modes.
3. **A lint framework** with configurable severity, per-workspace settings,
   and quick-fix generation.
4. **"Explain" features** showing pipeline stages, scope stacks, rewrite
   traces, and structural diffs.
5. **EdgeLorD integration** mapping structured diagnostics to LSP features:
   related information, code actions/quick fixes, hover explanations, and
   virtual "explain" documents.
6. **Snapshot testing** and CI enforcement ensuring diagnostic quality never
   regresses.

**Key principle:** every user-facing message must answer three questions:
*what happened*, *where it happened*, and *what to do about it*.

---

## 2. Vision and UX Principles (Section A)

### 2.1 Definition of "100% User-Friendly"

Measurable criteria:

| Criterion | Metric | Target |
|-----------|--------|--------|
| **Locatability** | % of diagnostics with a primary span pointing to real source | 100% (spanless errors use document-level span, never 0:0) |
| **Actionability** | % of errors with at least one hint or suggested fix | ≥ 80% |
| **Readability** | Flesch-Kincaid grade level of error messages | ≤ 10 (high school) |
| **Consistency** | % of diagnostics following the style guide template | 100% (enforced by CI) |
| **Determinism** | Identical input → identical output, byte-for-byte | 100% |
| **Latency** | Time from keystroke to diagnostics published | ≤ 200ms p95 |
| **Coverage** | % of error paths producing structured diagnostics | 100% |

### 2.2 Message Style Guide

**Voice:** Direct, neutral, specific. Never blame the user. Never say "you."
State what happened and what can be done.

**Structure of every diagnostic:**

```
[ML-E-003] unbound symbol `foo`
  --> src/example.comrade:4:12
  |
4 | (def bar (compose foo baz))
  |                  ^^^ not found in scope
  |
  = help: did you mean `f`? (defined at line 2)
  = help: add `(touch foo)` before this definition
  = note: visible bindings: f, baz, compose
```

**Template:**

```
[{CODE}] {TITLE}
  --> {FILE}:{LINE}:{COL}
  |
{N} | {SOURCE_LINE}
  |  {UNDERLINE} {LABEL}
  |
  = {SEVERITY}: {MESSAGE}
  [= help: {HINT}]*
  [= note: {CONTEXT}]*
```

**Vocabulary rules:**
- Use "expected X, found Y" for mismatches (never "got" or "received").
- Use "not found in scope" (never "undefined" or "does not exist").
- Use backticks for identifiers: `` `foo` ``.
- Use "add" / "remove" / "rename" in hints (never "try" or "consider").
- No emoji. No exclamation marks. No rhetorical questions.
- Abbreviations: do not abbreviate. Write "definition" not "def" in prose
  (backtick-quoted `def` for the keyword is fine).

**Severity levels:**
- **Error:** prevents successful elaboration. The program is rejected.
- **Warning:** accepted but likely wrong. Lint-detectable issues.
- **Information:** unsolved goals, informational annotations.
- **Hint:** style suggestions, naming conventions.

### 2.3 Perfect Diagnostic Template

Every `StructuredDiagnostic` carries these fields (details in Section 4):

```rust
pub struct StructuredDiagnostic {
    pub code: DiagnosticCode,           // ML-P-001, ML-E-003, etc.
    pub severity: Severity,             // Error, Warning, Information, Hint
    pub title: String,                  // One-line summary (no span info)
    pub primary: LabeledSpan,           // Main location + label
    pub related: Vec<LabeledSpan>,      // Secondary locations with labels
    pub message: String,                // Extended explanation (1-3 sentences)
    pub hints: Vec<String>,             // Actionable suggestions
    pub quick_fixes: Vec<QuickFix>,     // Machine-applicable edits
    pub explain_available: bool,        // Whether `explain` has deeper info
    pub notes: Vec<String>,             // Additional context (visible bindings, etc.)
}
```

---

## 3. Full Audit Methodology (Section B)

### 3.1 Step-by-Step Audit Process

To find every current error/warning path:

1. **Grep for error constructors.** Search all `Err(`, `::Reject {`, `Error {`,
   `Error::`, `SurfaceError::`, `MiniError {`, `ParseError::`, `MacroError::`,
   `ElaborationError::` across `new_surface_syntax` and `edgelord-lsp`.

2. **Grep for `format!` in error context.** Every `format!` producing a message
   string inside an error path. Flag any using `{:?}` (Debug dump) as
   "needs pretty-printer."

3. **Grep for `WorkspaceDiagnostic` construction.** Every site that builds a
   `WorkspaceDiagnostic` struct or calls `workspace_diagnostic_from_surface_error`.

4. **Grep for `Diagnostic {` in EdgeLorD.** Every LSP `Diagnostic` construction
   site. Check: does it set `related_information`? `code`? `data`?

5. **Grep for `Span::new(0` and `ByteSpan::new(0, 0)`.** Every fake span.

6. **Grep for `.unwrap()` and `panic!` in diagnostic paths.** These must not
   exist in user-facing code.

### 3.2 Current Error Taxonomy

| Phase | Error Type | Variants | Span Coverage | Message Quality |
|-------|-----------|----------|---------------|-----------------|
| **Parse** | `ParseError` | 8 | 6/8 (UnexpectedEof, OldParseError lack span) | Good structure, raw `{:?}` in some |
| **Expand** | `MacroError` | 5 | 5/5 (but 3 sites use dummy Span(0,0)) | Pattern/input shown as Debug dumps |
| **Elaborate** | `ElaborationError` | 5 | 5/5 | Good: name + span |
| **Elaborate** | `MorphismTerm::Reject` | 4 instances | 4/4 (inherits expr span) | Code+msg, but msg uses `{:?}` |
| **Backend** | `MiniError` | 6 kinds | 6/6 | Good: symbol name + explanation |
| **Query** | `SurfaceError::Query` | (string) | 0/∞ (never has span) | Vague: "unknown FileId" |
| **Import** | `SurfaceError::Import` | (string) | 0/∞ (never has span) | Debug dump of ImportError |
| **Kernel** | `SurfaceError::Kernel` | (string) | 0/∞ (uses `.render()`) | Depends on KernelValidationError |

**Total error variants:** 28+ distinct paths.
**Structured diagnostic coverage:** 0% (all use flat string messages).
**Fake span sites:** ≥ 6 (expand.rs template errors, EdgeLorD external commands).

### 3.3 Ensuring No Path Remains Raw

After this plan, every error path must produce a `StructuredDiagnostic`.
Enforcement:

1. **`WorkspaceDiagnostic` becomes `StructuredDiagnostic`.** The old type is
   replaced. Any code constructing the old type fails to compile.

2. **CI lint: no raw string diagnostics.** A `#[deny(raw_diagnostic)]` custom
   lint (or grep-based CI check) ensures no `WorkspaceDiagnostic { message: format!(...), ... }`
   escapes into the codebase without going through the structured constructor.

3. **CI lint: unique codes.** A test enumerates all `DiagnosticCode` variants
   and asserts each maps to a unique string. A second test greps the codebase
   for all code usages and asserts completeness.

4. **Snapshot tests.** Every error path has a snapshot test showing the
   rendered output. If someone adds a new error without a snapshot, CI fails.

### 3.4 Style Guide Enforcement in CI

A dedicated test (`test_diagnostic_style_compliance`) iterates all diagnostic
constructors and checks:

- Title does not contain a span (spans go in `primary` field).
- Title does not contain `{:?}` patterns.
- Title starts with a lowercase letter.
- Hints start with a verb ("add", "remove", "rename", "wrap").
- No emoji in any field.
- Message is ≤ 3 sentences.

---

## 4. Unified Diagnostics Architecture (Section C)

### 4.1 The `StructuredDiagnostic` Model

**Crate:** `new_surface_syntax`, in a new `src/diagnostic.rs` module.

```rust
use source_span::Span;
use serde::{Serialize, Deserialize};

/// A unique, stable diagnostic code.
/// Format: ML-{PHASE}-{NUMBER}
/// Phase: P (parse), M (macro), E (elaboration), R (rewrite),
///        L (lint), K (kernel), Q (query), B (backend)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosticCode {
    pub phase: Phase,
    pub number: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Parse,        // P
    Macro,        // M
    Elaboration,  // E
    Rewrite,      // R
    Lint,         // L
    Kernel,       // K
    Query,        // Q
    Backend,      // B
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A span with an attached human-readable label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabeledSpan {
    /// The byte range in source. `None` only for synthetic/unspannable items.
    pub span: Option<Span>,
    /// Human-readable label for this location (e.g., "not found in scope").
    pub label: String,
}

/// A machine-applicable edit suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickFix {
    /// Short description of the fix (e.g., "add `(touch foo)` before this def").
    pub title: String,
    /// Edits to apply. Each edit is (span_to_replace, replacement_text).
    /// An empty span means insertion at that point.
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    /// Byte range to replace. `Span::new(n, n)` means insert at offset n.
    pub span: Span,
    /// Replacement text.
    pub new_text: String,
}

/// The core diagnostic type used end-to-end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredDiagnostic {
    /// Unique stable code (e.g., ML-E-003).
    pub code: DiagnosticCode,
    /// Severity level.
    pub severity: Severity,
    /// One-line summary, no location info (e.g., "unbound symbol `foo`").
    pub title: String,
    /// Primary source location with label.
    pub primary: LabeledSpan,
    /// Secondary/related source locations with labels.
    pub related: Vec<LabeledSpan>,
    /// Extended explanation (1-3 sentences). May reference Mac Lane concepts.
    pub message: String,
    /// Actionable human-readable suggestions (each starts with a verb).
    pub hints: Vec<String>,
    /// Machine-applicable fix suggestions.
    pub quick_fixes: Vec<QuickFix>,
    /// Whether the "explain" command has additional information for this error.
    pub explain_available: bool,
    /// Additional context notes (visible bindings, expansion trace, etc.).
    pub notes: Vec<String>,
}
```

**Invariants:**
- `code` is unique across the entire codebase (enforced by test).
- `title` never contains span information (spans go in `primary`/`related`).
- `title` never uses `{:?}` Debug formatting.
- `primary.span` is `None` only for errors that genuinely have no source
  location (query errors, import errors referencing external files).
- `hints` entries each begin with an imperative verb.
- `quick_fixes` edits are valid: spans are within document bounds, non-overlapping.

**Serialization:** `serde` with `#[derive(Serialize, Deserialize)]`. Carried
in `WorkspaceReport` as `Vec<StructuredDiagnostic>`. Serialized to JSON for
LSP `Diagnostic.data` field (for rich client consumption).

### 4.2 Code Registry

All diagnostic codes defined in a single enum for uniqueness enforcement:

```rust
// In src/diagnostic.rs
impl DiagnosticCode {
    // Parse errors
    pub const UNEXPECTED_TOKEN:   Self = Self { phase: Phase::Parse, number: 1 };
    pub const UNEXPECTED_EOF:     Self = Self { phase: Phase::Parse, number: 2 };
    pub const INVALID_CHAR:       Self = Self { phase: Phase::Parse, number: 3 };
    pub const INVALID_NUMBER:     Self = Self { phase: Phase::Parse, number: 4 };
    pub const MISMATCHED_DELIM:   Self = Self { phase: Phase::Parse, number: 5 };
    pub const ILLEGAL_UNQUOTE:    Self = Self { phase: Phase::Parse, number: 6 };
    pub const ILLEGAL_SPLICE:     Self = Self { phase: Phase::Parse, number: 7 };
    pub const LEGACY_PARSE:       Self = Self { phase: Phase::Parse, number: 8 };

    // Macro errors
    pub const MATCH_FAILURE:      Self = Self { phase: Phase::Macro, number: 1 };
    pub const DUPLICATE_MACRO:    Self = Self { phase: Phase::Macro, number: 2 };
    pub const ILLEGAL_PATTERN_VAR:Self = Self { phase: Phase::Macro, number: 3 };
    pub const ILLEGAL_SPLICE_VAR: Self = Self { phase: Phase::Macro, number: 4 };
    pub const EXPANSION_DEPTH:    Self = Self { phase: Phase::Macro, number: 5 };

    // Elaboration errors
    pub const UNBOUND_SYMBOL:     Self = Self { phase: Phase::Elaboration, number: 1 };
    pub const INVALID_ARITY:      Self = Self { phase: Phase::Elaboration, number: 2 };
    pub const DUPLICATE_DEF:      Self = Self { phase: Phase::Elaboration, number: 3 };
    pub const INVALID_META:       Self = Self { phase: Phase::Elaboration, number: 4 };
    pub const RULE_NORMALIZATION: Self = Self { phase: Phase::Elaboration, number: 5 };
    pub const INVALID_HOLE:       Self = Self { phase: Phase::Elaboration, number: 6 };
    pub const EMPTY_APPLICATION:  Self = Self { phase: Phase::Elaboration, number: 7 };
    pub const SURFACE_IN_CORE:    Self = Self { phase: Phase::Elaboration, number: 8 };
    pub const LITERAL_UNSUPPORTED:Self = Self { phase: Phase::Elaboration, number: 9 };

    // Backend errors
    pub const DEF_WITHOUT_TOUCH:  Self = Self { phase: Phase::Backend, number: 1 };
    pub const DUPLICATE_TOUCH:    Self = Self { phase: Phase::Backend, number: 2 };
    pub const MALFORMED_RULE:     Self = Self { phase: Phase::Backend, number: 3 };
    pub const MALFORMED_SUGAR:    Self = Self { phase: Phase::Backend, number: 4 };

    // Lint warnings
    pub const UNUSED_TOUCH:       Self = Self { phase: Phase::Lint, number: 1 };
    pub const UNUSED_DEF:         Self = Self { phase: Phase::Lint, number: 2 };
    pub const SHADOWED_BINDING:   Self = Self { phase: Phase::Lint, number: 3 };
    pub const SUSPICIOUS_HOLE:    Self = Self { phase: Phase::Lint, number: 4 };
    pub const REDUNDANT_TOUCH:    Self = Self { phase: Phase::Lint, number: 5 };
    pub const RULE_IDENTITY:      Self = Self { phase: Phase::Lint, number: 6 };
    pub const AMBIGUOUS_SYMBOL:   Self = Self { phase: Phase::Lint, number: 7 };

    // Goal diagnostics
    pub const UNSOLVED_GOAL:      Self = Self { phase: Phase::Elaboration, number: 10 };

    // Query/import/kernel (no number gaps)
    pub const QUERY_ERROR:        Self = Self { phase: Phase::Query, number: 1 };
    pub const IMPORT_ERROR:       Self = Self { phase: Phase::Query, number: 2 };
    pub const KERNEL_ERROR:       Self = Self { phase: Phase::Kernel, number: 1 };

    /// Render as stable string: "ML-E-003"
    pub fn as_str(&self) -> String {
        let phase_char = match self.phase {
            Phase::Parse => 'P',
            Phase::Macro => 'M',
            Phase::Elaboration => 'E',
            Phase::Rewrite => 'R',
            Phase::Lint => 'L',
            Phase::Kernel => 'K',
            Phase::Query => 'Q',
            Phase::Backend => 'B',
        };
        format!("ML-{}-{:03}", phase_char, self.number)
    }
}
```

### 4.3 Conversion from Existing Error Types

Each existing error type gets a `to_diagnostic()` method:

```rust
impl ParseError {
    pub fn to_diagnostic(&self) -> StructuredDiagnostic {
        match self {
            ParseError::UnboundSymbol { name, span } => StructuredDiagnostic {
                code: DiagnosticCode::UNBOUND_SYMBOL,
                severity: Severity::Error,
                title: format!("unbound symbol `{}`", name),
                primary: LabeledSpan {
                    span: Some(*span),
                    label: "not found in scope".to_string(),
                },
                related: vec![],
                message: format!(
                    "The symbol `{}` is used here but has not been introduced \
                     with `(touch {})` in the current scope.",
                    name, name
                ),
                hints: vec![
                    format!("add `(touch {})` before this definition", name),
                ],
                quick_fixes: vec![],  // Quick fix added by EdgeLorD with insertion point
                explain_available: true,
                notes: vec![],  // Populated with visible bindings by caller
            },
            // ... other variants
        }
    }
}
```

(The above shows the pattern for `ElaborationError::UnboundSymbol`; every
variant of every error type gets the same treatment.)

### 4.4 Replacing WorkspaceDiagnostic

**Current:**
```rust
pub struct WorkspaceDiagnostic {
    pub message: String,
    pub span: Option<Span>,
    pub severity: WorkspaceDiagnosticSeverity,
    pub code: Option<&'static str>,
}
```

**New:** `WorkspaceReport.diagnostics` changes from `Vec<WorkspaceDiagnostic>`
to `Vec<StructuredDiagnostic>`. The old type is removed entirely.

Migration: `workspace_diagnostic_from_surface_error()` calls
`err.to_diagnostic()` instead of `format!("{}", err)`. The `surface_error_span`
and `surface_error_code` helper functions become unnecessary (the diagnostic
carries its own span and code).

### 4.5 Preserving Kernel Authority

`StructuredDiagnostic` is produced by the kernel (new_surface_syntax).
EdgeLorD may **augment** but not **replace** kernel diagnostics:

- **Augmentation:** EdgeLorD may add `hints` (e.g., "did you mean `bar`?"
  using fuzzy matching on known symbols), add `quick_fixes` (with concrete
  text edits based on document state), and add `notes` (e.g., listing
  visible bindings from its richer syntactic context).
- **No replacement:** EdgeLorD must not change `code`, `severity`, `title`,
  `primary`, or `message` of kernel-produced diagnostics.
- **Supplementary diagnostics:** EdgeLorD may produce its own
  `StructuredDiagnostic` instances for lints and syntactic checks (using
  `Phase::Lint` codes), but these are separate entries, not modifications
  of kernel diagnostics.

### 4.6 Multi-Cause Errors and Cycles

For errors with multiple contributing causes (e.g., a rule that fails
normalization because both LHS and RHS contain issues):

- `primary` points to the main error site.
- `related` contains labeled spans for each contributing cause.
- `message` explains the relationship.

For cycles (e.g., mutual recursion detected):

- `primary` points to one participant.
- `related` points to other participants with labels like
  "also part of cycle" or "references `foo` here."
- `notes` contains the full cycle path as a formatted string.

---

## 5. Spans, Source Maps, and "No Fake Spans" (Section D)

### 5.1 Span Architecture

**Existing:** `source_span::Span` is `{ start: usize, end: usize }` — byte
offsets into a source string.

**No changes to `source_span::Span` itself.** Instead, the diagnostic layer
uses `Option<Span>` to explicitly represent the absence of a span.

### 5.2 The "No Fake Spans" Rule

**Hard invariant:** No code may construct `Span::new(0, 0)` as a placeholder
for "I don't have a span." Instead:

- `LabeledSpan.span` is `Option<Span>`. When `None`, the span is explicitly
  absent and EdgeLorD renders it as a document-level diagnostic (no underline).
- `HoleContextEntry` (from the hole occurrence plan) has no span field
  (already established in v3).
- `Binding.span` in EdgeLorD is `Option<ByteSpan>` (already established in v3).

**Enforcement:**
- CI grep: `rg 'Span::new\(0\s*,\s*0\)' --type rust` must return zero hits
  outside of test code explicitly annotated `// FAKE_SPAN_TEST`.
- CI grep: `rg 'ByteSpan::new\(0\s*,\s*0\)' --type rust` — same rule.

### 5.3 Attaching Spans to Binders and Scope Entries

The elaborator's `Scope` currently stores `BTreeSet<String>` for binders and
`BTreeMap<String, CoreForm>` for defs — no span information.

**This plan does not change `Scope` to add spans.** Rationale: the elaborator
processes expanded forms where binder spans may not correspond to original
source. Instead:

- EdgeLorD's syntactic layer (document.rs) provides binder spans for
  `touch`/`def`/`let` forms detected in the original CST.
- Kernel-derived `HoleContextEntry` honestly reports no span.
- The pretty-printer for scope/context displays handles `None` spans gracefully
  (shows the name without a location link).

### 5.4 Macro Expansion Span Strategy

When macro expansion synthesizes new code, the resulting `SExpr` nodes have
spans that point into the original source (the macro invocation site). Current
behavior at expand.rs lines 402-435 uses `Span::new(0, 0)` for some
synthesized nodes.

**Fix:** All synthesized `SExpr` nodes during macro expansion must carry the
span of the **macro invocation** (the outermost list of the macro call). This
is the `expr.span` available at the point where `apply_macros` is called.

**Implementation:** In `apply_template()` (expand.rs), replace every
`Span::new(0, 0)` with the `invocation_span` parameter (the span of the
macro call site). This requires threading an `invocation_span: Span` parameter
through `apply_template` and its recursive calls.

**Result:** Errors in expanded code point to the macro invocation, not to
`0:0`. The `related` span in the diagnostic can point to the macro definition
(if available).

### 5.5 Spans for Currently Spanless Errors

| Error | Current | Fix |
|-------|---------|-----|
| `ParseError::UnexpectedEof` | No span | Use `Span::new(text.len(), text.len())` — points to end of file |
| `ParseError::OldParseError` | No span | Attempt to extract span from inner error; if not available, use `None` (document-level) |
| `SurfaceError::Import` | No span | Attach span of the `(use ...)` form that triggered the import |
| `SurfaceError::Kernel` | No span | Attach span from the form being validated (passed as parameter) |
| `SurfaceError::Query` | No span | These are infrastructure errors; use `None` (document-level) |

### 5.6 Testing the No-Fake-Spans Invariant

```rust
#[test]
fn no_fake_spans_in_diagnostics() {
    let sources = vec![
        "(def x ?h)",        // elab error: no touch
        "(",                 // parse error: unexpected EOF
        "(sugar m () ())",   // macro error
        "(def x (quote y))", // surface-in-core
    ];
    for source in sources {
        let result = compile_surface(source, &CompileOptions::default());
        match result {
            Err(err) => {
                let diag = err.to_diagnostic();
                if let Some(span) = diag.primary.span {
                    assert_ne!((span.start, span.end), (0, 0),
                        "Diagnostic for {:?} has fake span (0,0)", source);
                }
                // None is acceptable — it means honestly unspanned
            }
            Ok(_) => {} // no error, fine
        }
    }
}
```

---

## 6. Pretty-Printing System (Section E)

### 6.1 Architecture: Wadler-Lindig Doc Model

A `Doc` type (algebraic document) supports width-aware layout:

```rust
// In new_surface_syntax src/pretty.rs

/// Width-aware document for pretty-printing.
#[derive(Debug, Clone)]
pub enum Doc {
    /// Empty document.
    Nil,
    /// Literal text (no newlines).
    Text(String),
    /// Concatenation.
    Cat(Box<Doc>, Box<Doc>),
    /// Newline or space (depending on layout mode).
    Line,
    /// Increase indentation by `n` for the inner doc.
    Nest(usize, Box<Doc>),
    /// Choose between flat (single-line) and broken (multi-line) layout.
    Group(Box<Doc>),
}

impl Doc {
    pub fn text(s: impl Into<String>) -> Self { Doc::Text(s.into()) }
    pub fn nil() -> Self { Doc::Nil }
    pub fn line() -> Self { Doc::Line }
    pub fn nest(n: usize, d: Doc) -> Self { Doc::Nest(n, Box::new(d)) }
    pub fn group(d: Doc) -> Self { Doc::Group(Box::new(d)) }
    pub fn cat(a: Doc, b: Doc) -> Self { Doc::Cat(Box::new(a), Box::new(b)) }

    /// Convenience: separate items with a separator doc.
    pub fn intersperse(sep: Doc, docs: Vec<Doc>) -> Doc { /* ... */ }

    /// Render to string with given max width.
    pub fn render(&self, width: usize) -> String { /* Wadler-Lindig algorithm */ }

    /// Render flat (single-line, ignoring Line breaks).
    pub fn render_flat(&self) -> String { /* ... */ }
}
```

### 6.2 Printers for Each Type

Each type gets a `fn to_doc(&self) -> Doc` method (or free function).

**SExpr:**
```rust
pub fn sexpr_to_doc(expr: &SExpr) -> Doc {
    match &expr.kind {
        SExprKind::Atom(Atom::Symbol(s)) => Doc::text(s),
        SExprKind::Atom(Atom::Integer(n)) => Doc::text(n.to_string()),
        SExprKind::Atom(Atom::String(s)) => Doc::text(format!("\"{}\"", s)),
        SExprKind::List(items) => {
            let inner = Doc::intersperse(
                Doc::cat(Doc::text(","), Doc::line()),
                items.iter().map(sexpr_to_doc).collect(),
            );
            Doc::group(Doc::cat(
                Doc::text("("),
                Doc::cat(Doc::nest(2, inner), Doc::text(")")),
            ))
        }
        SExprKind::Quote(inner) => Doc::cat(Doc::text("'"), sexpr_to_doc(inner)),
        // ... QuasiQuote, Unquote, UnquoteSplicing
    }
}
```

**MorphismTerm:**
```rust
pub fn morphism_term_to_doc(term: &MorphismTerm) -> Doc {
    match term {
        MorphismTerm::Generator { id, inputs, outputs } =>
            Doc::text(format!("gen({}, {} -> {})",
                id.0, inputs.len(), outputs.len())),
        MorphismTerm::Compose { components, .. } => {
            let parts = components.iter().map(morphism_term_to_doc).collect();
            Doc::group(Doc::cat(
                Doc::text("(compose"),
                Doc::cat(
                    Doc::nest(2, Doc::cat(Doc::line(), Doc::intersperse(Doc::line(), parts))),
                    Doc::text(")"),
                ),
            ))
        }
        MorphismTerm::Hole(id) => Doc::text(format!("?{}", id)),
        MorphismTerm::Reject { code, msg } =>
            Doc::text(format!("<reject:{}: {}>", code, msg)),
        MorphismTerm::App { op, args, .. } =>
            Doc::text(format!("(app {} [{}])", op.0, args.len())),
        MorphismTerm::InDoctrine { doctrine, term } => Doc::cat(
            Doc::text(format!("(in-doctrine {:?} ", doctrine)),
            Doc::cat(morphism_term_to_doc(term), Doc::text(")")),
        ),
    }
}
```

**CoreForm:**
```rust
pub fn core_form_to_doc(form: &CoreForm) -> Doc {
    match form {
        CoreForm::Begin(forms) => {
            let inner = forms.iter().map(core_form_to_doc).collect();
            Doc::group(Doc::cat(
                Doc::text("(begin"),
                Doc::cat(
                    Doc::nest(2, Doc::cat(Doc::line(), Doc::intersperse(Doc::line(), inner))),
                    Doc::text(")"),
                ),
            ))
        }
        CoreForm::Touch(name) => Doc::text(format!("(touch {})", name)),
        CoreForm::Def(name, term) => Doc::group(Doc::cat(
            Doc::text(format!("(def {} ", name)),
            Doc::cat(Doc::nest(2, morphism_term_to_doc(term)), Doc::text(")")),
        )),
        CoreForm::Rule(lhs, rhs, meta) => Doc::group(Doc::cat(
            Doc::text("(rule"),
            Doc::cat(
                Doc::nest(2, Doc::cat(
                    Doc::line(),
                    Doc::cat(morphism_term_to_doc(lhs), Doc::cat(Doc::line(), morphism_term_to_doc(rhs))),
                )),
                Doc::text(")"),
            ),
        )),
    }
}
```

**Scope/Context:**
```rust
pub fn scope_to_doc(binders: &BTreeSet<String>, defs: &BTreeMap<String, CoreForm>) -> Doc {
    let entries: Vec<Doc> = binders.iter().map(|name| {
        if defs.contains_key(name) {
            Doc::text(format!("def {}", name))
        } else {
            Doc::text(format!("touch {}", name))
        }
    }).collect();
    if entries.is_empty() {
        Doc::text("(empty scope)")
    } else {
        Doc::intersperse(Doc::cat(Doc::text(","), Doc::line()), entries)
    }
}
```

**Rewrite trace:**
```rust
pub fn rewrite_step_to_doc(step_number: usize, rule_name: &str,
                            before: &MorphismTerm, after: &MorphismTerm) -> Doc {
    Doc::cat(
        Doc::text(format!("step {}: apply `{}`", step_number, rule_name)),
        Doc::nest(4, Doc::cat(
            Doc::cat(Doc::line(), Doc::cat(Doc::text("before: "), morphism_term_to_doc(before))),
            Doc::cat(Doc::line(), Doc::cat(Doc::text("after:  "), morphism_term_to_doc(after))),
        )),
    )
}
```

### 6.3 Two Modes

Every printer supports two rendering widths:

- **Compact** (inline): `render_flat()` — single line, for inline display,
  inlay hints, hover one-liners.
- **Explanatory** (multi-line): `render(80)` — 80-column, for diagnostics,
  explain panels, virtual documents.

### 6.4 Determinism and Snapshot Testing

All printers are pure functions of their input (no hidden state, no allocation
order dependencies). Snapshot tests:

```rust
#[test]
fn snapshot_sexpr_pretty() {
    let expr = parse_one("(def foo (compose (bar ?x) (baz ?y)))");
    let rendered = sexpr_to_doc(&expr).render(40);
    insta::assert_snapshot!(rendered, @r###"
    (def foo
      (compose
        (bar ?x)
        (baz ?y)))
    "###);
}
```

Use the `insta` crate for snapshot testing. Each printer gets ≥ 5 snapshot
tests covering: empty input, single atom, nested list, wide output (fits one
line), narrow output (broken across lines).

### 6.5 No Color Dependency

All rendered output is plain text. Color is **not** added at the printer
level. If a UI surface wants color (e.g., terminal diagnostics), it applies
ANSI codes after rendering, keyed on structural annotations. The `Doc` type
does not carry color information. This ensures screen-reader accessibility
and plain-text log compatibility.

---

## 7. Linting System (Section F)

### 7.1 Lint Registry

| Code | Name | Severity | Description | Quick Fix |
|------|------|----------|-------------|-----------|
| ML-L-001 | `unused-touch` | Warning | `(touch x)` with no subsequent `(def x ...)` | Remove the `(touch x)` form |
| ML-L-002 | `unused-def` | Warning | `(def x ...)` where `x` never appears in later terms/rules | (none — user intent unclear) |
| ML-L-003 | `shadowed-binding` | Warning | `(touch x)` when `x` is already in scope | Rename to avoid conflict |
| ML-L-004 | `suspicious-hole` | Hint | `?x` appears in a non-term context (e.g., as a macro argument) | (none — informational) |
| ML-L-005 | `redundant-touch` | Warning | `(touch x)` immediately followed by another `(touch x)` | Remove the duplicate |
| ML-L-006 | `rule-identity` | Warning | `(rule lhs rhs ...)` where LHS and RHS are structurally identical | Remove the trivial rule |
| ML-L-007 | `ambiguous-symbol` | Hint | A bare symbol in term position could be confused with a hole | Use `?` prefix for intentional holes |

### 7.2 Lint Framework

```rust
// In new_surface_syntax src/lint.rs

/// Trait for implementing a lint check.
pub trait Lint {
    /// Unique lint code.
    fn code(&self) -> DiagnosticCode;

    /// Human-readable lint name.
    fn name(&self) -> &'static str;

    /// Default severity (can be overridden by config).
    fn default_severity(&self) -> Severity;

    /// Run the lint on a compiled bundle, producing diagnostics.
    fn check(
        &self,
        source: &str,
        bundle: &CoreBundleV0,
        forms: &[SExpr],           // expanded forms for span access
    ) -> Vec<StructuredDiagnostic>;
}

/// Registry of all built-in lints.
pub fn builtin_lints() -> Vec<Box<dyn Lint>> {
    vec![
        Box::new(UnusedTouchLint),
        Box::new(UnusedDefLint),
        Box::new(ShadowedBindingLint),
        Box::new(SuspiciousHoleLint),
        Box::new(RedundantTouchLint),
        Box::new(RuleIdentityLint),
        Box::new(AmbiguousSymbolLint),
    ]
}

/// Run all enabled lints and collect diagnostics.
pub fn run_lints(
    source: &str,
    bundle: &CoreBundleV0,
    forms: &[SExpr],
    config: &LintConfig,
) -> Vec<StructuredDiagnostic> {
    builtin_lints()
        .iter()
        .filter(|lint| config.is_enabled(lint.code()))
        .flat_map(|lint| {
            let mut diags = lint.check(source, bundle, forms);
            // Override severity from config
            for d in &mut diags {
                if let Some(override_sev) = config.severity_override(lint.code()) {
                    d.severity = override_sev;
                }
            }
            diags
        })
        .collect()
}
```

### 7.3 Lint Configuration

```rust
pub struct LintConfig {
    /// Per-lint severity overrides. Key is code string (e.g., "ML-L-001").
    pub overrides: BTreeMap<String, LintLevel>,
}

pub enum LintLevel {
    Allow,   // Suppress entirely
    Warn,    // Show as warning
    Deny,    // Show as error (compilation still succeeds)
    Default, // Use the lint's built-in default
}
```

Configuration sources (in priority order):
1. **Inline pragmas:** `; lint:allow unused-touch` comments (future, not in v1)
2. **Workspace config:** `.edgelord.toml` file with `[lints]` section
3. **Defaults:** from `Lint::default_severity()`

For v1, only workspace config and defaults. Inline pragmas are a natural
extension but require comment parsing (defer comment parsing only, not the
lint framework itself).

### 7.4 Example Lint Implementation

```rust
struct UnusedTouchLint;

impl Lint for UnusedTouchLint {
    fn code(&self) -> DiagnosticCode { DiagnosticCode::UNUSED_TOUCH }
    fn name(&self) -> &'static str { "unused-touch" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        _source: &str,
        bundle: &CoreBundleV0,
        _forms: &[SExpr],
    ) -> Vec<StructuredDiagnostic> {
        // Collect all touched names and all defined names from CoreForms
        let mut touched: BTreeMap<String, Span> = BTreeMap::new();
        let mut defined: BTreeSet<String> = BTreeSet::new();

        fn scan_forms(forms: &[CoreForm], touched: &mut BTreeMap<String, Span>,
                      defined: &mut BTreeSet<String>) {
            for form in forms {
                match form {
                    CoreForm::Touch(name) => { /* record with span */ }
                    CoreForm::Def(name, _) => { defined.insert(name.clone()); }
                    CoreForm::Begin(inner) => scan_forms(inner, touched, defined),
                    CoreForm::Rule(..) => {}
                }
            }
        }

        scan_forms(&bundle.forms, &mut touched, &mut defined);

        touched.iter()
            .filter(|(name, _)| !defined.contains(name.as_str()))
            .map(|(name, span)| StructuredDiagnostic {
                code: DiagnosticCode::UNUSED_TOUCH,
                severity: Severity::Warning,
                title: format!("unused touch `{}`", name),
                primary: LabeledSpan {
                    span: Some(*span),
                    label: "introduced here but never defined".to_string(),
                },
                related: vec![],
                message: format!(
                    "The symbol `{}` was introduced with `(touch {})` but no \
                     `(def {} ...)` follows in this scope.",
                    name, name, name
                ),
                hints: vec![
                    format!("remove `(touch {})` if it is not needed", name),
                    format!("add `(def {} ...)` to provide a definition", name),
                ],
                quick_fixes: vec![QuickFix {
                    title: format!("remove `(touch {})`", name),
                    edits: vec![TextEdit {
                        span: *span,
                        new_text: String::new(), // delete
                    }],
                }],
                explain_available: false,
                notes: vec![],
            })
            .collect()
    }
}
```

### 7.5 Preventing False Positives

Each lint must document its false-positive strategy:

- **unused-touch:** Not triggered if the touch appears inside a `begin` block
  with a corresponding def in the same block. Not triggered if the name
  appears in any rule LHS/RHS within the same scope.
- **shadowed-binding:** Not triggered for `_` prefixed names (convention
  for intentional shadowing).
- **rule-identity:** Not triggered if the rule has `(meta debug ...)` tags
  (intentional identity rule for tracing).

Each lint has ≥ 3 tests:
1. A positive case (lint fires).
2. A negative case (lint does not fire on valid code).
3. A false-positive-prevention case (lint does not fire in documented exception).

---

## 8. Debugging and "Explain" Features (Section G)

### 8.1 "Explain This Error" Command

When a diagnostic has `explain_available: true`, EdgeLorD exposes an
"Explain this error" code action. Activating it opens a virtual document
(via `workspace/applyEdit` with a `file://` URI to a temporary file, or
via a custom `edgelord/explain` method).

The explain output shows:

1. **Error summary** — the diagnostic title and message.
2. **Pipeline stage** — which phase produced the error (parse/expand/elab/etc.).
3. **Intermediate representations** — the relevant forms at each stage:
   - Source text (with span highlighted)
   - After macro expansion (if applicable)
   - Core form (if elaboration reached that point)
4. **Scope state** — full scope at the error site (all binders and defs).
5. **Similar names** — fuzzy matches for unbound symbols.
6. **Relevant documentation** — links to Mac Lane language reference for
   the failing construct.

### 8.2 Scope Stack Visualization

```
Scope at error site (line 12, col 5):
  1. touch x        (line 1)
  2. touch y        (line 2)
  3. def x = ...    (line 3)
  4. touch z        (line 8)
  ─────────────────────────
  Looking for: `foo`
  Not found. Similar: `f` (line 6), `for` (imported)
```

Implementation: The elaborator's `snapshot_context()` (from the hole
occurrence plan) provides the binder list. EdgeLorD augments with spans
from its syntactic analysis. The pretty-printer formats the scope as
shown above.

### 8.3 Rewrite Trace Display

When a rewrite fails or produces unexpected results, the explain feature
shows the application trace:

```
Rewrite trace for `(rule (compose f g) (compose g f))`:
  Step 1: Match LHS pattern against focus
    Pattern: (compose ?a ?b)
    Focus:   (compose (id x) (proj y))
    Binding: ?a = (id x), ?b = (proj y)

  Step 2: Construct RHS from bindings
    Template: (compose ?b ?a)
    Result:   (compose (proj y) (id x))

  Step 3: Boundary check
    Source boundary: [x] -> [y]
    Result boundary: [y] -> [x]    ← MISMATCH
    Error: rewrite would change the boundary (source ≠ target)
```

### 8.4 "Expected vs Found" Structural Diff

For type mismatches and arity errors, show a structural diff:

```
Type mismatch in composition:

  Expected (source of second morphism):
    (obj A)

  Found (target of first morphism):
    (obj B)

  These must be equal for composition to be valid.
  See Mac Lane Ch. I.3: "composition g ∘ f requires cod(f) = dom(g)."
```

For complex terms, the diff highlights the point of divergence:

```
  Expected: (compose (f ·) (g (h x)))
  Found:    (compose (f ·) (g (h y)))
                                  ^
  Difference at position 3.1.1: `x` vs `y`
```

### 8.5 EdgeLorD UI Surfaces

| Feature | LSP Mechanism | Trigger |
|---------|--------------|---------|
| Explain panel | Code action → virtual document | Click "Explain" on diagnostic |
| Scope at cursor | Hover | Hover over any identifier |
| Rewrite preview | Code action resolve | Hover over rewrite action |
| Term diff | Hover on mismatch diagnostic | Automatic |
| Binding origin | Hover on identifier | Shows definition site |

---

## 9. EdgeLorD Integration Plan (Section H)

### 9.1 WorkspaceReport Changes

`WorkspaceReport` gains:
```rust
pub struct WorkspaceReport {
    pub diagnostics: Vec<StructuredDiagnostic>,  // CHANGED from Vec<WorkspaceDiagnostic>
    pub fingerprint: Option<[u8; 32]>,
    pub revision: u64,
    pub bundle: Option<CoreBundleV0>,
    pub holes: Vec<HoleOccurrence>,              // from hole occurrence plan
    pub lint_diagnostics: Vec<StructuredDiagnostic>,  // NEW: lints run separately
}
```

### 9.2 Mapping to LSP Diagnostics

```rust
fn structured_to_lsp(
    diag: &StructuredDiagnostic,
    text: &str,
) -> lsp_types::Diagnostic {
    let range = match diag.primary.span {
        Some(span) => byte_span_to_range(text, ByteSpan::new(span.start, span.end)),
        None => Range::new(Position::new(0, 0), Position::new(0, 0)),
    };

    let related_information = if diag.related.is_empty() {
        None
    } else {
        Some(diag.related.iter().filter_map(|r| {
            r.span.map(|span| DiagnosticRelatedInformation {
                location: Location {
                    uri: current_uri.clone(),
                    range: byte_span_to_range(text, ByteSpan::new(span.start, span.end)),
                },
                message: r.label.clone(),
            })
        }).collect())
    };

    lsp_types::Diagnostic {
        range,
        severity: Some(match diag.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Information => DiagnosticSeverity::INFORMATION,
            Severity::Hint => DiagnosticSeverity::HINT,
        }),
        code: Some(NumberOrString::String(diag.code.as_str())),
        code_description: None,
        source: Some("mac-lane".to_string()),
        message: format!("{}\n\n{}", diag.title, diag.message),
        related_information,
        tags: None,
        data: Some(serde_json::to_value(diag).unwrap()),
    }
}
```

Key points:
- `Diagnostic.data` carries the full `StructuredDiagnostic` as JSON for
  rich client consumption.
- `related_information` maps `LabeledSpan` entries to LSP related info.
- `message` combines `title` and `message` (LSP has no separate title field).
- `source` is `"mac-lane"` (consistent branding).

### 9.3 Quick Fixes as Code Actions

When a diagnostic has `quick_fixes`, EdgeLorD generates `CodeAction` entries:

```rust
fn quick_fix_to_code_action(
    diag: &StructuredDiagnostic,
    fix: &QuickFix,
    uri: &Url,
    text: &str,
) -> CodeAction {
    let edits: Vec<lsp_types::TextEdit> = fix.edits.iter().map(|e| {
        lsp_types::TextEdit {
            range: byte_span_to_range(text, ByteSpan::new(e.span.start, e.span.end)),
            new_text: e.new_text.clone(),
        }
    }).collect();

    CodeAction {
        title: fix.title.clone(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![structured_to_lsp(diag, text)]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), edits)])),
            ..Default::default()
        }),
        ..Default::default()
    }
}
```

### 9.4 Graceful Degradation for Partial Parse States

When parsing fails, the kernel returns an error but EdgeLorD still has the
previous `ParsedDocument` (from syntactic parsing, which is more lenient).
The strategy:

1. If the kernel report has diagnostics, show them.
2. If the kernel report has no bundle (compilation failed), fall back to
   syntactic goals from `ParsedDocument.goals`.
3. Lint diagnostics are only produced when the kernel succeeds (lint checks
   require a valid `CoreBundleV0`).
4. Hover and inlay hints use whatever goals are available (kernel or syntactic).

### 9.5 Unifying Kernel and Syntactic Goals

(This is already solved in the Hole Occurrence Plan v3. Summary: kernel goals
have identity `(owner, ordinal)` with `goal:defname:N` IDs; syntactic goals
have `goal-start-end-name` IDs. The hybrid strategy uses kernel goals when
available, syntactic fallback otherwise.)

---

## 10. Implementation Plan with Gates (Section I)

### Gate 1: Diagnostic Types and Pretty-Printer Foundation (Kernel)

**What:** Add `StructuredDiagnostic`, `DiagnosticCode`, `LabeledSpan`,
`QuickFix`, `TextEdit`, `Severity`, `Phase` types. Add `Doc` pretty-printer
model with `render()` and `render_flat()`. Add `sexpr_to_doc()` printer.

**Files touched:**
| File | Change |
|------|--------|
| `new_surface_syntax/src/diagnostic.rs` | NEW: all diagnostic types |
| `new_surface_syntax/src/pretty.rs` | NEW: Doc model + SExpr printer |
| `new_surface_syntax/src/lib.rs` | Add `pub mod diagnostic; pub mod pretty;` |

**Tests:**
- Unit: `DiagnosticCode::as_str()` produces correct format for all codes.
- Unit: All codes are unique (iterate registry, check for duplicates).
- Snapshot: `sexpr_to_doc` for 5 representative expressions.
- Unit: `Doc::render()` handles empty, single-line, multi-line cases.

**Gate test:** `cargo check -p new_surface_syntax` and all existing tests pass.

### Gate 2: Term Printers (Kernel)

**What:** Add pretty-printers for `CoreForm`, `MorphismTerm`, scope/context,
`CompiledRule`, `Meta`.

**Files touched:**
| File | Change |
|------|--------|
| `new_surface_syntax/src/pretty.rs` | Add printers for all term types |

**Tests:**
- Snapshot: 5 tests per printer (empty, atom, nested, wide, narrow).
- Unit: Compact mode (render_flat) produces single line.
- Unit: Printers are deterministic (same input → same output).

**Gate test:** 25+ snapshot tests pass.

### Gate 3: Error-to-Diagnostic Conversion (Kernel)

**What:** Add `to_diagnostic()` method to `ParseError`, `MacroError`,
`ElaborationError`, `SurfaceError`, `MiniError`. Add `to_diagnostic()` for
`MorphismTerm::Reject`. Replace `WorkspaceDiagnostic` with
`StructuredDiagnostic` in `WorkspaceReport`.

**Files touched:**
| File | Change |
|------|--------|
| `src/error.rs` | Add `to_diagnostic()` impl for each error type |
| `src/elaborate.rs` | Reject paths produce StructuredDiagnostic |
| `src/comrade_workspace.rs` | Replace WorkspaceDiagnostic with StructuredDiagnostic |
| `src/comrade_workspace.rs` | Remove `workspace_diagnostic_from_surface_error` |
| `src/mini_backend.rs` | Add `to_diagnostic()` for MiniError |
| `src/lib.rs` | Export new diagnostic types |

**Tests:**
- Unit: Every error variant's `to_diagnostic()` produces a valid diagnostic
  with non-empty title, correct code, and correct severity.
- Unit: No diagnostic has `title` containing `{:?}`.
- Unit: No diagnostic has `primary.span == Some(Span::new(0, 0))`.
- Snapshot: 10 representative error→diagnostic conversions.

**Gate test:** All existing tests pass (with `WorkspaceDiagnostic` migration).

### Gate 4: Fix Macro Expansion Spans (Kernel)

**What:** Thread `invocation_span` through `apply_template` in expand.rs.
Remove all `Span::new(0, 0)` from macro expansion.

**Files touched:**
| File | Change |
|------|--------|
| `src/expand.rs` | Thread invocation_span through apply_template |

**Tests:**
- Unit: Macro expansion error diagnostics have `primary.span` pointing to
  invocation site (not 0:0).
- Regression: All existing expand tests pass.

**Gate test:** CI grep for `Span::new(0, 0)` returns 0 hits outside test code.

### Gate 5: Lint Framework (Kernel)

**What:** Add `Lint` trait, `LintConfig`, `run_lints()`, and 7 built-in lints.
Wire lints into `elaborate_query` or `ComradeWorkspace` report path.

**Files touched:**
| File | Change |
|------|--------|
| `src/lint.rs` | NEW: Lint trait, LintConfig, run_lints, 7 lint impls |
| `src/comrade_workspace.rs` | Add `lint_diagnostics` to WorkspaceReport |
| `src/lib.rs` | Add `pub mod lint;`, export LintConfig |

**Tests:**
- Per lint: 3 tests (positive, negative, false-positive-prevention) = 21 tests.
- Unit: `run_lints` with all lints disabled produces empty vec.
- Unit: `run_lints` with `LintConfig` overrides respects severity changes.

**Gate test:** 23+ lint tests pass. All existing tests pass.

### Gate 6: EdgeLorD Structured Diagnostics (EdgeLorD)

**What:** Update EdgeLorD to consume `StructuredDiagnostic`. Map to LSP
`Diagnostic` with `related_information` and `data`. Generate code actions
for quick fixes. Update all `WorkspaceReport` construction sites.

**Files touched:**
| File | Change |
|------|--------|
| `src/lsp.rs` | Replace workspace_report_to_diagnostics with structured mapping |
| `src/lsp.rs` | Add quick_fix_to_code_action |
| `src/lsp.rs` | Update document_diagnostics_from_report |
| `src/lsp.rs` | Update code_action to include quick fixes from diagnostics |
| `src/lsp.rs` | Update workspace_error_report for new WorkspaceReport shape |
| `src/lsp.rs` | Remove old WorkspaceDiagnostic mapping code |
| `src/proof_session.rs` | Update for new WorkspaceReport fields |

**Tests:**
- Unit: `structured_to_lsp` produces correct LSP diagnostic fields.
- Unit: Quick fixes produce valid code actions with correct edit ranges.
- Integration: didOpen with error-producing text → publishDiagnostics
  contains structured code, related_information, and data.

**Gate test:** All EdgeLorD tests pass. Integration test verifies structured fields.

### Gate 7: Hover and Explain Upgrades (EdgeLorD)

**What:** Upgrade hover to show pretty-printed context with the new printers.
Add "Explain this error" code action. Add scope visualization in hover.

**Files touched:**
| File | Change |
|------|--------|
| `src/lsp.rs` | Rewrite hover() to use pretty-printers |
| `src/lsp.rs` | Add explain code action handler |
| `src/lsp.rs` | Add scope_at_cursor for hover augmentation |

**Tests:**
- Snapshot: 5 hover output tests with representative programs.
- Unit: Explain action produces well-formatted virtual document.
- Unit: Scope visualization shows correct bindings.

**Gate test:** All tests pass. Manual verification in Helix.

### Gate 8: Snapshot Test Harness and CI (Both)

**What:** Add `insta` dependency. Add snapshot test suite covering all
printers, all diagnostic conversions, representative hover outputs.
Add CI enforcement scripts.

**Files touched:**
| File | Change |
|------|--------|
| `new_surface_syntax/Cargo.toml` | Add `insta` dev-dependency |
| `edgelord-lsp/Cargo.toml` | Add `insta` dev-dependency |
| `tests/` | New snapshot test files |
| CI config | Add `cargo insta test`, grep checks |

**Tests:**
- 50+ snapshot tests across both crates.
- CI: `rg 'Span::new\(0.*0\)' --type rust` returns 0 non-test hits.
- CI: `rg '\{:\?\}' src/error.rs src/elaborate.rs src/expand.rs` returns 0 hits.
- CI: All diagnostic codes are unique (test).

**Gate test:** CI passes. No snapshot regressions.

---

## 11. Success Metrics and Acceptance Tests (Section J)

### 11.1 User-Facing Acceptance Tests

| Test | Input | Required Output Contains |
|------|-------|------------------------|
| Unbound symbol | `(def x ?h)` | `"unbound symbol"`, `"touch"` hint, span on `?h` or `x` |
| Duplicate def | `(touch x)\n(def x 1)\n(def x 2)` | `"duplicate definition"`, span on second def, related span on first |
| Invalid arity | `(touch a)\n(def a b c)` | `"invalid arity"`, `"expected 2, found 3"` or similar |
| Mismatched paren | `(def x (foo)` | `"unexpected end"` or `"mismatched"`, span near EOF |
| Unused touch | `(touch x)\n(touch y)\n(def y 1)` | `"unused touch \`x\`"` warning |
| Macro match fail | `(sugar m \`(m ?x) \`(foo ?x))\n(m)` | `"match failure"`, pattern + input shown |
| Unsolved goal | `(touch a)\n(def a ?h)` | `"unsolved goal"` information diagnostic |
| Explain available | `(def x ?h)` | diagnostic has `explain_available: true` |
| Quick fix: remove touch | `(touch unused)` | Code action: "remove `(touch unused)`" |
| Quick fix: add touch | `(def x ?h)` | Hint: "add `(touch x)` before this definition" |

### 11.2 Quality Metrics

| Metric | Target |
|--------|--------|
| Diagnostic coverage (% of error paths with StructuredDiagnostic) | 100% |
| Span coverage (% of diagnostics with primary span) | ≥ 90% |
| Hint coverage (% of errors with ≥ 1 hint) | ≥ 80% |
| Quick fix coverage (% of lint warnings with a quick fix) | ≥ 60% |
| Snapshot test count | ≥ 50 |
| Diagnostic code uniqueness | 100% (enforced by test) |
| No Debug dumps in user messages | 100% (enforced by CI grep) |
| No fake spans (0,0) | 100% (enforced by CI grep) |

### 11.3 Performance Budgets

| Operation | Budget |
|-----------|--------|
| Full diagnostic pipeline (parse + expand + elab + lint + format) | ≤ 50ms for ≤ 1000-line file |
| Pretty-print one MorphismTerm | ≤ 1ms |
| Pretty-print full diagnostic message | ≤ 5ms |
| Incremental diagnostic update (after keystroke) | ≤ 200ms total (including debounce) |
| Memory per open document (diagnostics + lint state) | ≤ 10 MB |

### 11.4 Definition of Done Checklist

- [ ] All 28+ error variants have `to_diagnostic()` implementations.
- [ ] All diagnostics use `DiagnosticCode` (no raw strings).
- [ ] All diagnostics follow the style guide (enforced by test).
- [ ] Pretty-printers exist for: SExpr, CoreForm, MorphismTerm, scope,
      context, CompiledRule, Meta.
- [ ] All 7 lints implemented with 3+ tests each.
- [ ] EdgeLorD maps structured diagnostics to LSP with related_information.
- [ ] Quick fixes produce valid code actions.
- [ ] "Explain this error" code action works for ≥ 5 error types.
- [ ] Hover shows pretty-printed context (not raw strings).
- [ ] 50+ snapshot tests pass.
- [ ] CI grep for fake spans returns 0 hits.
- [ ] CI grep for Debug dumps in error messages returns 0 hits.
- [ ] All diagnostic codes are unique (test passes).
- [ ] Performance budgets met.
- [ ] Manual verification in Helix: diagnostics appear with codes,
      hover shows rich context, code actions include quick fixes.

---

## 12. Concrete Examples (Section K)

### 12.1 Before vs After Diagnostics (10 examples)

**Example 1: Unbound symbol**

Before:
```
elaboration error: unbound symbol 'foo' at Span { start: 22, end: 25 }
```

After:
```
[ML-E-001] unbound symbol `foo`
  --> test.comrade:3:12
  |
3 | (def bar (compose foo baz))
  |                   ^^^ not found in scope
  |
  = help: add `(touch foo)` before this definition
  = note: visible bindings: bar, baz, compose
```

**Example 2: Duplicate definition**

Before:
```
elaboration error: duplicate definition 'x' at Span { start: 28, end: 29 }
```

After:
```
[ML-E-003] duplicate definition `x`
  --> test.comrade:4:6
  |
2 | (def x (id a))
  |      - first definition here
  ...
4 | (def x (id b))
  |      ^ `x` is already defined in this scope
  |
  = help: rename one of the definitions to avoid the conflict
```

**Example 3: Missing touch**

Before:
```
elaboration error: unbound symbol 'x' at Span { start: 5, end: 6 }
```

After:
```
[ML-E-001] unbound symbol `x`
  --> test.comrade:1:6
  |
1 | (def x ?goal)
  |      ^ not found in scope
  |
  = help: add `(touch x)` before this definition
  = note: `(def name term)` requires a prior `(touch name)` in the same scope
```

**Example 4: Unexpected end of file**

Before:
```
parse error: unexpected end of input: expected )
```

After:
```
[ML-P-002] unexpected end of input
  --> test.comrade:3:1
  |
1 | (def foo
2 |   (compose bar
  |                ^ expected `)` to close this list
  |
  = help: add the missing `)` — there are 2 unclosed parentheses
```

**Example 5: Invalid arity**

Before:
```
elaboration error: invalid arity for 'def': expected 2, found 1 at Span { start: 0, end: 8 }
```

After:
```
[ML-E-002] invalid arity for `def`
  --> test.comrade:1:1
  |
1 | (def foo)
  | ^^^^^^^^^ expected 2 arguments, found 1
  |
  = note: `(def name term)` requires a name and a term
  = help: add the missing term: `(def foo ?goal)`
```

**Example 6: Macro match failure**

Before:
```
macro error: macro match failure: pattern=List([Atom("m"), Hole("x")]) input=List([Atom("m")]) at Span { start: 40, end: 43 }
```

After:
```
[ML-M-001] macro `m` does not match this invocation
  --> test.comrade:2:1
  |
1 | (sugar m `(m ?x) `(foo ?x))
  |           ------- pattern expects 1 argument
2 | (m)
  | ^^^ no arguments provided
  |
  = note: pattern `(m ?x)` requires exactly 1 argument
  = help: provide an argument: `(m some_value)`
```

**Example 7: Unused touch (lint)**

Before:
```
(no warning — not detected)
```

After:
```
[ML-L-001] unused touch `x`
  --> test.comrade:1:1
  |
1 | (touch x)
  | ^^^^^^^^^ introduced here but never defined
  |
  = help: remove `(touch x)` if it is not needed
  = help: add `(def x ...)` to provide a definition
```

**Example 8: Illegal unquote**

Before:
```
parse error: illegal unquote outside quasiquote at Span { start: 5, end: 7 }
```

After:
```
[ML-P-006] illegal unquote outside quasiquote
  --> test.comrade:1:6
  |
1 | (foo ,x bar)
  |      ^^ unquote `,` can only appear inside a quasiquote
  |
  = note: quasiquote syntax: `(form ,expr) where `,` splices `expr`
  = help: wrap the containing expression in a quasiquote if this is a template
```

**Example 9: Expansion depth exceeded**

Before:
```
macro error: macro expansion depth 100 exceeded at Span { start: 0, end: 12 }
```

After:
```
[ML-M-005] macro expansion depth limit reached
  --> test.comrade:1:1
  |
1 | (loop-forever)
  | ^^^^^^^^^^^^^^ expansion reached 100 levels (limit)
  |
  = note: this usually means a macro calls itself without a base case
  = help: ensure the macro has a non-recursive branch for its base case
```

**Example 10: Unsolved goal**

Before:
```
(shown as syntactic inlay hint only, no diagnostic)
```

After:
```
[ML-E-010] unsolved goal `h` in def `example`
  --> test.comrade:3:15
  |
3 | (def example ?h)
  |              ^^ this hole needs a proof term
  |
  = note: goal context: touch a, def b
  = note: target type: unknown (type inference not yet available)
```

### 12.2 "Explain This Error" Examples (5)

**Explain 1: Unbound symbol**
```
═══ Explain: [ML-E-001] unbound symbol `foo` ═══

Pipeline stage: Elaboration (after macro expansion)

Source:
  1 | (touch bar)
  2 | (def bar (compose foo baz))
                        ^^^ error here

Scope at error site:
  touch bar       (line 1)
  ──────────────
  `foo` is not in scope.

Similar names in scope:
  (none found)

What happened:
  The elaborator encountered the symbol `foo` in the body of
  `(def bar ...)` but `foo` has not been introduced with `(touch foo)`
  in any enclosing scope.

  In Mac Lane, every symbol must be explicitly declared before use.
  This is the "scope discipline": `(touch name)` introduces a name,
  then `(def name term)` gives it a definition.

To fix:
  Add `(touch foo)` before this definition, then either:
  - Define it: `(def foo some_term)`
  - Use it as a hole: replace `foo` with `?foo`
```

**Explain 2: Duplicate definition**
```
═══ Explain: [ML-E-003] duplicate definition `x` ═══

Pipeline stage: Elaboration

Source:
  1 | (touch x)
  2 | (def x (id a))    ← first definition
  3 | (touch y)
  4 | (def x (id b))    ← second definition (ERROR)

Scope at line 4:
  touch x       (line 1)  ← already defined at line 2
  def x = ...   (line 2)
  touch y       (line 3)

What happened:
  `x` was already defined by the `(def x ...)` on line 2. Mac Lane's
  scope discipline forbids redefining a name in the same scope.

  Unlike some languages, Mac Lane does not allow shadowing within
  a single scope level. Each name can be defined at most once per
  scope.

To fix:
  - Rename one of the definitions (e.g., `x2`)
  - Use `(begin ...)` to create a nested scope for the second definition
```

**Explain 3: Macro match failure**
```
═══ Explain: [ML-M-001] macro `m` does not match ═══

Pipeline stage: Macro expansion

Macro definition (line 1):
  (sugar m `(m ?x) `(foo ?x))
  Pattern: (m ?x)  — expects 1 argument after `m`

Invocation (line 2):
  (m)  — provides 0 arguments

Match trace:
  Step 1: Match head symbol `m` = `m` ✓
  Step 2: Match ?x against <nothing> ✗ — no more elements

What happened:
  The macro `m` was defined with a pattern that expects one argument
  (`?x`), but the invocation `(m)` provides no arguments.

To fix:
  Provide an argument: `(m some_value)`
```

**Explain 4: Invalid arity for rule**
```
═══ Explain: [ML-E-002] invalid arity for `rule` ═══

Pipeline stage: Elaboration

Source:
  5 | (rule (foo ?x) (bar ?x))

What happened:
  `(rule lhs rhs meta)` requires exactly 3 arguments:
    1. lhs  — the left-hand side pattern
    2. rhs  — the right-hand side replacement
    3. meta — metadata (provenance, class tags)

  This invocation provides only 2 arguments (lhs and rhs), missing
  the required metadata.

To fix:
  Add metadata: `(rule (foo ?x) (bar ?x) (meta provenance))`
```

**Explain 5: Surface syntax in core**
```
═══ Explain: [ML-E-008] quote not allowed in elaborated core ═══

Pipeline stage: Elaboration (sexpr_to_morphism)

Source:
  3 | (def example '(some form))
                    ^^^^^^^^^^^^

What happened:
  The elaborator converts surface s-expressions into kernel terms
  (MorphismTerm). Quote forms (`'(...)`) are surface-level syntax
  used in macro definitions and should be eliminated during macro
  expansion before elaboration.

  Finding a quote in elaboration means either:
  - A macro failed to expand properly
  - A quote was used outside a macro context

Pipeline trace:
  Parse:    '(some form) → Quote(List([some, form]))
  Expand:   (no macro matched — quote passes through)
  Elab:     Quote → REJECT (surface syntax in core)

To fix:
  - If this is a macro template, use `(sugar ...)` to define the macro
  - If you meant a literal list, remove the quote: `(some form)`
```

### 12.3 Pretty-Printed Contexts and Scopes (5 examples)

**Example 1: Simple scope**
```
Scope:
  touch a
  touch b
  def a = (compose (f x) (g y))
```

**Example 2: Nested scope with begin**
```
Scope:
  touch x                           (line 1)
  def x = (id obj_a)               (line 2)
  ─── begin block (line 4) ───
    touch y                         (line 5)
    def y = (compose x (f ?goal))   (line 6)
```

**Example 3: Goal context (from hole occurrence)**
```
Goal `?h` in def `example`:
  context:
    touch a
    touch b
    def a = <defined>
  target: unknown
  id: goal:example:0
```

**Example 4: Empty scope**
```
Scope: (empty)
```

**Example 5: Large scope (truncated)**
```
Scope (12 bindings):
  touch a, touch b, def a, touch c, def b, touch d, def c, touch e
  … +4 more bindings
```

### 12.4 Rewrite Mismatch Diffs (5 examples)

**Example 1: Boundary mismatch**
```
Rewrite rejected: boundary mismatch

  Rule: (rule (compose ?f ?g) (compose ?g ?f) (meta swap))

  Before: (compose (mor_a : [X] -> [Y]) (mor_b : [Y] -> [Z]))
  After:  (compose (mor_b : [Y] -> [Z]) (mor_a : [X] -> [Y]))

  Source boundary: [X] -> [Z]
  Result boundary: [Y] -> [Y]
                   ^^^     ^^^  different!

  Swapping non-commutative morphisms changes the boundary.
```

**Example 2: Pattern does not match focus**
```
Rule `simp_id` does not match:
  Pattern: (compose (id ?x) ?f)
  Focus:   (compose (proj a) (inj b))
                    ^^^^^^^^
  Mismatch: expected `(id ?)`, found `(proj a)`
```

**Example 3: Variable binding conflict**
```
Rule application failed: binding conflict

  Pattern: (compose ?f (compose ?f ?g))
  Focus:   (compose a (compose b c))

  ?f binds to `a` at position 1
  ?f binds to `b` at position 2.1
  Conflict: `a` ≠ `b`
```

**Example 4: Arity mismatch in application**
```
Rule does not apply: arity mismatch

  Pattern: (app op ?x ?y)     — expects 2 arguments
  Focus:   (app op a b c)     — has 3 arguments
```

**Example 5: Successful rewrite (for reference)**
```
Rewrite applied: `simp_id_left`
  Before: (compose (id x) (f x y))
  After:  (f x y)
  Rule:   (compose (id ?a) ?f) → ?f
  Binding: ?a = x, ?f = (f x y)
```

---

## 13. Final Checklist

1. **Section A (Vision):** Style guide defined, template provided, measurable
   criteria specified.
2. **Section B (Audit):** Step-by-step process, taxonomy of 28+ error paths,
   enforcement via CI.
3. **Section C (Architecture):** `StructuredDiagnostic` with all fields,
   code registry, serialization strategy, kernel authority model.
4. **Section D (Spans):** No-fake-spans invariant, macro span fix,
   spanless-error fixes, enforcement by CI grep.
5. **Section E (Pretty-printing):** Wadler-Lindig Doc model, printers for
   all term types, compact/explanatory modes, snapshot tests.
6. **Section F (Linting):** 7 lints with trait framework, configuration,
   quick fixes, false-positive prevention, 21+ tests.
7. **Section G (Explain):** Explain command, scope visualization, rewrite
   traces, structural diffs, UI surfaces.
8. **Section H (EdgeLorD):** WorkspaceReport migration, LSP mapping with
   related_information and data, quick-fix code actions, graceful
   degradation.
9. **Section I (Gates):** 8 gates with exact files, APIs, tests, and
   success criteria.
10. **Section J (Metrics):** 10 acceptance tests, quality metrics with
    targets, performance budgets, definition-of-done checklist.
11. **Section K (Examples):** 10 before/after diagnostics, 5 explain
    outputs, 5 scope displays, 5 rewrite diffs.
12. **No deferred work.** Every item is specified with concrete types,
    algorithms, and tests.
