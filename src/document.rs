use std::{cmp::Ordering, collections::BTreeSet};
use serde::{Deserialize, Serialize};

use comrade_lisp::{
    ParseError, parser,
    syntax::{Atom, SExpr, SExprKind},
};
use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn contains_offset(&self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub message: String,
    pub span: ByteSpan,
}

#[derive(Debug, Clone)]
struct CstNode {
    span: ByteSpan,
    parent: Option<usize>,
    children: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub text: String,
    pub diagnostics: Vec<ParseDiagnostic>,
    pub goals: Vec<Goal>,
    nodes: Vec<CstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goal {
    pub goal_id: String,
    pub stable_id: Option<String>,
    pub name: Option<String>,
    pub span: ByteSpan,
    pub context: Vec<Binding>,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingKind {
    Let,
    Touch,
    Def,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub name: String,
    pub kind: BindingKind,
    pub span: ByteSpan,
    pub value_preview: Option<String>,
    pub ty_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalInlayHint {
    pub goal_id: String,
    pub offset: usize,
    pub label: String,
}

impl ParsedDocument {
    pub fn parse(text: String) -> Self {
        let text_len = text.len();
        let mut nodes = vec![CstNode {
            span: ByteSpan::new(0, text_len),
            parent: None,
            children: Vec::new(),
        }];

        let mut diagnostics = Vec::new();
        let mut goals = Vec::new();

        match parser::parse_module(&text) {
            Ok(module) => {
                goals = extract_goals(&module.body);
                for form in module.body {
                    add_expr_node(&mut nodes, 0, &form);
                }
            }
            Err(err) => {
                diagnostics.push(ParseDiagnostic {
                    message: format!("{}", err),
                    span: parse_error_span(&err, text_len),
                });
            }
        }

        Self {
            text,
            diagnostics,
            goals,
            nodes,
        }
    }

    pub fn selection_chain_for_offset(&self, offset: usize) -> Vec<ByteSpan> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut current = 0usize;
        loop {
            let next = self.nodes[current]
                .children
                .iter()
                .copied()
                .filter(|child| self.nodes[*child].span.contains_offset(offset))
                .min_by(node_order(&self.nodes));
            match next {
                Some(child) => current = child,
                None => break,
            }
        }

        let mut chain = Vec::new();
        let mut cursor = Some(current);
        while let Some(id) = cursor {
            chain.push(self.nodes[id].span);
            cursor = self.nodes[id].parent;
        }

        let mut deduped = Vec::new();
        for span in chain {
            if deduped.last().copied() != Some(span) {
                deduped.push(span);
            }
        }
        deduped
    }

    pub fn goal_at_offset(&self, offset: usize) -> Option<&Goal> {
        self.goals
            .iter()
            .find(|goal| goal.span.contains_offset(offset))
    }

    /// Return the byte offset at the **end** of each top-level form.
    ///
    /// These are the valid checked-boundary positions for proof stepping
    /// (SB0). The list is deterministically sorted and deduplicated.
    ///
    /// **INV D-*:** deterministic — nodes are added in source order in
    /// `add_expr_node`, so `nodes[0].children` iteration order is stable.
    pub fn top_level_form_boundaries(&self) -> Vec<usize> {
        if self.nodes.len() <= 1 {
            return Vec::new();
        }
        let mut ends: Vec<usize> = self.nodes[0]
            .children
            .iter()
            .map(|&c| self.nodes[c].span.end)
            .collect();
        ends.sort_unstable();
        ends.dedup();
        ends
    }

    pub fn goal_inlay_hints_in_range(&self, range: ByteSpan) -> Vec<GoalInlayHint> {
        let mut hints = self
            .goals
            .iter()
            .filter(|goal| spans_intersect(goal.span, range))
            .map(|goal| GoalInlayHint {
                goal_id: goal.goal_id.clone(),
                offset: goal.span.end,
                label: goal_label(goal, &self.text),
            })
            .collect::<Vec<_>>();

        hints.sort_by(|a, b| {
            a.offset
                .cmp(&b.offset)
                .then(a.goal_id.cmp(&b.goal_id))
                .then(a.label.cmp(&b.label))
        });
        hints
    }
}

fn node_order(nodes: &[CstNode]) -> impl FnMut(&usize, &usize) -> Ordering + '_ {
    move |a, b| {
        let lhs = nodes[*a].span;
        let rhs = nodes[*b].span;
        lhs.len()
            .cmp(&rhs.len())
            .then(lhs.start.cmp(&rhs.start))
            .then(lhs.end.cmp(&rhs.end))
            .then(a.cmp(b))
    }
}

fn add_expr_node(nodes: &mut Vec<CstNode>, parent: usize, expr: &SExpr) -> usize {
    let span = expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
    let id = nodes.len();
    nodes.push(CstNode {
        span,
        parent: Some(parent),
        children: Vec::new(),
    });
    nodes[parent].children.push(id);

    match &expr.kind {
        SExprKind::List(items) => {
            for item in items {
                add_expr_node(nodes, id, item);
            }
        }
        SExprKind::Quote(inner)
        | SExprKind::QuasiQuote(inner)
        | SExprKind::Unquote(inner)
        | SExprKind::UnquoteSplicing(inner) => {
            add_expr_node(nodes, id, inner);
        }
        SExprKind::Atom(_) => {}
    }

    id
}

fn parse_error_span(err: &ParseError, text_len: usize) -> ByteSpan {
    match err {
        ParseError::UnexpectedToken { span, .. }
        | ParseError::InvalidChar { span, .. }
        | ParseError::InvalidNumber { span }
        | ParseError::MismatchedDelimiter { span, .. }
        | ParseError::IllegalUnquote { span }
        | ParseError::IllegalSplice { span } => span.as_ref().map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0)),
        ParseError::UnexpectedEof { .. } => ByteSpan::new(text_len, text_len),
        ParseError::InternalError { .. } => ByteSpan::new(0, 0),
        ParseError::OldParseError(_) => ByteSpan::new(0, 0),
    }
}

fn extract_goals(forms: &[SExpr]) -> Vec<Goal> {
    let mut goals = Vec::new();
    let mut top_level_bindings = Vec::<Binding>::new();

    for form in forms {
        let mut local_frames = Vec::new();
        collect_holes_in_expr(form, &mut local_frames, &top_level_bindings, &mut goals);
        collect_top_level_bindings(form, &mut top_level_bindings);
    }

    goals
}

fn collect_holes_in_expr(
    expr: &SExpr,
    local_frames: &mut Vec<Vec<Binding>>,
    top_level_bindings: &[Binding],
    out: &mut Vec<Goal>,
) {
    match &expr.kind {
        SExprKind::Atom(Atom::Symbol(name)) if name.starts_with('?') => {
            // Unbound variable starting with ? is a hole
            out.push(Goal {
                goal_id: goal_id(expr.span.map(|s| s.start).unwrap_or(0), expr.span.map(|s| s.end).unwrap_or(0), Some(name.as_str())),
                stable_id: None,
                name: Some(name.clone()),
                span: expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0)),
                context: merged_context(local_frames, top_level_bindings),
                target: "unknown".to_string(),
            });
        }
        SExprKind::List(items) => {
            if let Some(name) = hole_form_name(items) {
                out.push(Goal {
                    goal_id: goal_id(expr.span.map(|s| s.start).unwrap_or(0), expr.span.map(|s| s.end).unwrap_or(0), Some(name.as_str())),
                    stable_id: None,
                    name: Some(name),
                    span: expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0)),
                    context: merged_context(local_frames, top_level_bindings),
                    target: "unknown".to_string(),
                });
            }
            if let Some(frame) = let_bindings(items) {
                // Traverse let value expression outside local binder scope.
                if items.len() >= 3 {
                    collect_holes_in_expr(&items[2], local_frames, top_level_bindings, out);
                }
                local_frames.push(frame);
                for item in items.iter().skip(3) {
                    collect_holes_in_expr(item, local_frames, top_level_bindings, out);
                }
                local_frames.pop();
            } else {
                for item in items {
                    collect_holes_in_expr(item, local_frames, top_level_bindings, out);
                }
            }
        }
        SExprKind::Quote(inner)
        | SExprKind::QuasiQuote(inner)
        | SExprKind::Unquote(inner)
        | SExprKind::UnquoteSplicing(inner) => {
            collect_holes_in_expr(inner, local_frames, top_level_bindings, out)
        }
        SExprKind::Atom(_) => {}
    }
}

fn hole_form_name(items: &[SExpr]) -> Option<String> {
    if items.len() < 2 {
        return None;
    }
    let SExprKind::Atom(Atom::Symbol(head)) = &items[0].kind else {
        return None;
    };
    if head != "hole" {
        return None;
    }
    match &items[1].kind {
        SExprKind::Atom(Atom::Symbol(name)) => Some(name.clone()),
        _ => Some("anonymous".to_string()),
    }
}

fn collect_top_level_bindings(form: &SExpr, context: &mut Vec<Binding>) {
    let SExprKind::List(items) = &form.kind else {
        return;
    };
    if items.len() < 2 {
        return;
    }
    let SExprKind::Atom(Atom::Symbol(head)) = &items[0].kind else {
        return;
    };
    if head != "touch" && head != "def" {
        return;
    }
    let SExprKind::Atom(Atom::Symbol(name)) = &items[1].kind else {
        return;
    };
    if context.iter().all(|b| b.name != *name) {
        context.push(Binding {
            name: name.clone(),
            kind: if head == "touch" {
                BindingKind::Touch
            } else {
                BindingKind::Def
            },
            span: items[1].span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0)),
            value_preview: None,
            ty_preview: None,
        });
    }
}

fn goal_id(start: usize, end: usize, name: Option<&str>) -> String {
    let n = name.unwrap_or("anon");
    format!("goal-{start}-{end}-{n}")
}

fn spans_intersect(a: ByteSpan, b: ByteSpan) -> bool {
    if b.start == b.end {
        return a.contains_offset(b.start);
    }
    a.start < b.end && b.start < a.end
}

fn goal_label(goal: &Goal, text: &str) -> String {
    let name = goal.name.as_deref().unwrap_or("?");
    let slice = text.get(goal.span.start..goal.span.end).unwrap_or_default();
    if slice.trim_start().starts_with("(hole") {
        format!("hole {name} : {}", goal.target)
    } else {
        format!("?{name} : {}", goal.target)
    }
}

fn let_bindings(items: &[SExpr]) -> Option<Vec<Binding>> {
    if items.len() < 4 {
        return None;
    }
    let SExprKind::Atom(Atom::Symbol(head)) = &items[0].kind else {
        return None;
    };
    if head != "let" {
        return None;
    }

    let mut bindings = Vec::new();
    match &items[1].kind {
        SExprKind::Atom(Atom::Symbol(name)) => bindings.push(Binding {
            name: name.clone(),
            kind: BindingKind::Let,
            span: items[1].span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0)),
            value_preview: None,
            ty_preview: None,
        }),
        SExprKind::List(names) => {
            for name_expr in names {
                if let SExprKind::Atom(Atom::Symbol(name)) = &name_expr.kind {
                    bindings.push(Binding {
                        name: name.clone(),
                        kind: BindingKind::Let,
                        span: name_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0)),
                        value_preview: None,
                        ty_preview: None,
                    });
                }
            }
        }
        _ => {}
    }
    if bindings.is_empty() {
        return None;
    }
    Some(bindings)
}

fn merged_context(local_frames: &[Vec<Binding>], top_level_bindings: &[Binding]) -> Vec<Binding> {
    let mut context = Vec::new();
    let mut seen = BTreeSet::new();

    for frame in local_frames.iter().rev() {
        for binding in frame {
            if seen.insert(binding.name.clone()) {
                context.push(binding.clone());
            }
        }
    }
    for binding in top_level_bindings {
        if seen.insert(binding.name.clone()) {
            context.push(binding.clone());
        }
    }

    context
}

pub fn position_to_offset(text: &str, position: tower_lsp::lsp_types::Position) -> usize {
    let mut line = 0u32;
    let mut col_utf16 = 0u32;
    let mut last_boundary = 0usize;

    for (idx, ch) in text.char_indices() {
        if line == position.line && col_utf16 >= position.character {
            return idx;
        }

        if ch == '\n' {
            if line == position.line {
                return idx;
            }
            line += 1;
            col_utf16 = 0;
            last_boundary = idx + ch.len_utf8();
            continue;
        }

        if line == position.line {
            col_utf16 += ch.len_utf16() as u32;
            if col_utf16 >= position.character {
                return idx + ch.len_utf8();
            }
        }

        last_boundary = idx + ch.len_utf8();
    }

    if line == position.line {
        return text.len();
    }

    last_boundary
}

pub fn offset_to_position(text: &str, offset: usize) -> tower_lsp::lsp_types::Position {
    let mut line = 0u32;
    let mut col_utf16 = 0u32;

    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            return tower_lsp::lsp_types::Position::new(line, col_utf16);
        }

        if ch == '\n' {
            line += 1;
            col_utf16 = 0;
        } else {
            col_utf16 += ch.len_utf16() as u32;
        }
    }

    tower_lsp::lsp_types::Position::new(line, col_utf16)
}

pub fn apply_content_changes(text: &str, changes: &[TextDocumentContentChangeEvent]) -> String {
    let mut out = text.to_string();
    for change in changes {
        // LSP content changes are applied in the order received.
        if let Some(range) = change.range {
            let start = position_to_offset(&out, range.start);
            let end = position_to_offset(&out, range.end);
            let safe_start = start.min(out.len());
            let safe_end = end.min(out.len()).max(safe_start);
            out.replace_range(safe_start..safe_end, &change.text);
        } else {
            out = change.text.clone();
        }
    }
    out
}

pub fn selection_chain_is_well_formed(chain: &[ByteSpan]) -> bool {
    if chain.is_empty() {
        return false;
    }
    for spans in chain.windows(2) {
        let inner = spans[0];
        let outer = spans[1];
        if inner.start < outer.start || inner.end > outer.end {
            return false;
        }
    }
    true
}

pub fn top_level_symbols(text: &str) -> Vec<(String, ByteSpan)> {
    let Ok(module) = parser::parse_module(text) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for form in module.body {
        let span = form.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
        let label = match &form.kind {
            SExprKind::List(items) => match items.first() {
                Some(head) => match &head.kind {
                    SExprKind::Atom(atom) => format!("{}", atom),
                    _ => "list".to_string(),
                },
                None => "list".to_string(),
            },
            SExprKind::Atom(atom) => format!("{}", atom),
            SExprKind::Quote(_) => "quote".to_string(),
            SExprKind::QuasiQuote(_) => "quasiquote".to_string(),
            SExprKind::Unquote(_) => "unquote".to_string(),
            SExprKind::UnquoteSplicing(_) => "unquote-splicing".to_string(),
        };
        out.push((label, span));
    }
    out
}

// ── Symbol Index ────────────────────────────────────────────────────

/// The kind of definition a symbol represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolDefKind {
    Touch,
    Def,
    Rule,
    Sugar,
    Let,
    Lambda,
    Import,
}

/// A symbol definition in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDef {
    pub name: String,
    pub kind: SymbolDefKind,
    /// Span of the name itself (for goto-definition targets)
    pub name_span: ByteSpan,
    /// Span of the entire form (for document symbols)
    pub form_span: ByteSpan,
    /// Optional detail (e.g. the form head or type annotation preview)
    pub detail: Option<String>,
}

/// A reference (usage) of a symbol in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRef {
    pub name: String,
    pub span: ByteSpan,
}

/// Complete symbol index for a document — definitions + references.
#[derive(Debug, Clone)]
pub struct SymbolIndex {
    /// All definitions in the document (sorted by position).
    pub definitions: Vec<SymbolDef>,
    /// All symbol references/usages in the document (sorted by position).
    pub references: Vec<SymbolRef>,
}

impl SymbolIndex {
    /// Build a symbol index from document text.
    pub fn build(text: &str) -> Self {
        let mut definitions = Vec::new();
        let mut references = Vec::new();

        if let Ok(module) = parser::parse_module(text) {
            for form in &module.body {
                index_sexpr(form, false, &mut definitions, &mut references);
            }
        }

        // INV D-*: deterministic ordering
        definitions.sort_by_key(|d| d.name_span.start);
        references.sort_by_key(|r| r.span.start);

        SymbolIndex { definitions, references }
    }

    /// Find the definition of a symbol by name.
    pub fn find_definition(&self, name: &str) -> Option<&SymbolDef> {
        self.definitions.iter().find(|d| d.name == name)
    }

    /// Find all references to a symbol by name.
    pub fn find_references(&self, name: &str) -> Vec<&SymbolRef> {
        self.references.iter().filter(|r| r.name == name).collect()
    }

    /// Find the definition at a given offset (cursor on definition name).
    pub fn definition_at_offset(&self, offset: usize) -> Option<&SymbolDef> {
        self.definitions.iter().find(|d| d.name_span.contains_offset(offset))
    }

    /// Find the reference at a given offset (cursor on a usage).
    pub fn reference_at_offset(&self, offset: usize) -> Option<&SymbolRef> {
        self.references.iter().find(|r| r.span.contains_offset(offset))
    }

    /// Get all unique symbol names that are defined in this document.
    pub fn defined_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.definitions.iter().map(|d| d.name.as_str()).collect();
        names.dedup();
        names
    }

    /// Get completion candidates: all definitions + referenced names (deduplicated).
    pub fn completion_candidates(&self) -> Vec<(&str, Option<SymbolDefKind>)> {
        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();

        for def in &self.definitions {
            if seen.insert(def.name.as_str()) {
                candidates.push((def.name.as_str(), Some(def.kind)));
            }
        }
        for r in &self.references {
            if seen.insert(r.name.as_str()) {
                candidates.push((r.name.as_str(), None));
            }
        }
        candidates
    }
}

/// Recursively walk an s-expression collecting definitions and references.
fn index_sexpr(
    expr: &SExpr,
    in_quote: bool,
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<SymbolRef>,
) {
    let form_span = expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));

    match &expr.kind {
        SExprKind::List(items) if !in_quote => {
            let head_str = items.first().and_then(|h| {
                if let SExprKind::Atom(Atom::Symbol(s)) = &h.kind { Some(s.as_str()) } else { None }
            });

            match head_str {
                // (def name body) / (define name body)
                Some("def") | Some("define") | Some("define-facet") => {
                    if let Some(name_expr) = items.get(1) {
                        if let SExprKind::Atom(Atom::Symbol(name)) = &name_expr.kind {
                            let name_span = name_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                            defs.push(SymbolDef {
                                name: name.clone(),
                                kind: SymbolDefKind::Def,
                                name_span,
                                form_span,
                                detail: Some("def".to_string()),
                            });
                        }
                    }
                    for item in items.iter().skip(2) {
                        index_sexpr(item, false, defs, refs);
                    }
                }

                // (touch name [type])
                Some("touch") => {
                    if let Some(name_expr) = items.get(1) {
                        if let SExprKind::Atom(Atom::Symbol(name)) = &name_expr.kind {
                            let name_span = name_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                            defs.push(SymbolDef {
                                name: name.clone(),
                                kind: SymbolDefKind::Touch,
                                name_span,
                                form_span,
                                detail: Some("touch".to_string()),
                            });
                        }
                    }
                    for item in items.iter().skip(2) {
                        index_sexpr(item, false, defs, refs);
                    }
                }

                // (rule name LHS RHS [meta])
                Some("rule") => {
                    if let Some(name_expr) = items.get(1) {
                        if let SExprKind::Atom(Atom::Symbol(name)) = &name_expr.kind {
                            let name_span = name_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                            defs.push(SymbolDef {
                                name: name.clone(),
                                kind: SymbolDefKind::Rule,
                                name_span,
                                form_span,
                                detail: Some("rule".to_string()),
                            });
                        }
                    }
                    for item in items.iter().skip(2) {
                        index_sexpr(item, false, defs, refs);
                    }
                }

                // (sugar name pattern template)
                Some("sugar") => {
                    if let Some(name_expr) = items.get(1) {
                        if let SExprKind::Atom(Atom::Symbol(name)) = &name_expr.kind {
                            let name_span = name_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                            defs.push(SymbolDef {
                                name: name.clone(),
                                kind: SymbolDefKind::Sugar,
                                name_span,
                                form_span,
                                detail: Some("sugar".to_string()),
                            });
                        }
                    }
                    for item in items.iter().skip(2) {
                        index_sexpr(item, false, defs, refs);
                    }
                }

                // (use Module::Path symbol [as alias])
                Some("use") => {
                    // The imported symbol is a definition in this file's scope
                    if let Some(sym_expr) = items.get(2) {
                        if let SExprKind::Atom(Atom::Symbol(name)) = &sym_expr.kind {
                            let name_span = sym_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                            defs.push(SymbolDef {
                                name: name.clone(),
                                kind: SymbolDefKind::Import,
                                name_span,
                                form_span,
                                detail: items.get(1).and_then(|m| {
                                    if let SExprKind::Atom(Atom::Symbol(s)) = &m.kind { Some(format!("use {}", s)) } else { None }
                                }),
                            });
                        }
                    }
                    // Check for alias: (use M::P sym as alias)
                    if items.len() > 4 {
                        if let (Some(as_kw), Some(alias_expr)) = (items.get(3), items.get(4)) {
                            if let SExprKind::Atom(Atom::Symbol(kw)) = &as_kw.kind {
                                if kw == "as" {
                                    if let SExprKind::Atom(Atom::Symbol(alias)) = &alias_expr.kind {
                                        let alias_span = alias_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                                        defs.push(SymbolDef {
                                            name: alias.clone(),
                                            kind: SymbolDefKind::Import,
                                            name_span: alias_span,
                                            form_span,
                                            detail: Some(format!("alias")),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // (let (name val ...) body) or (let name val body)
                Some("let") => {
                    if let Some(bindings_expr) = items.get(1) {
                        match &bindings_expr.kind {
                            SExprKind::List(bindings) => {
                                for (i, b) in bindings.iter().enumerate() {
                                    if i % 2 == 0 {
                                        if let SExprKind::Atom(Atom::Symbol(name)) = &b.kind {
                                            let name_span = b.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                                            defs.push(SymbolDef {
                                                name: name.clone(),
                                                kind: SymbolDefKind::Let,
                                                name_span,
                                                form_span,
                                                detail: Some("let".to_string()),
                                            });
                                        }
                                    } else {
                                        index_sexpr(b, false, defs, refs);
                                    }
                                }
                            }
                            SExprKind::Atom(Atom::Symbol(name)) => {
                                let name_span = bindings_expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                                defs.push(SymbolDef {
                                    name: name.clone(),
                                    kind: SymbolDefKind::Let,
                                    name_span,
                                    form_span,
                                    detail: Some("let".to_string()),
                                });
                            }
                            _ => {}
                        }
                    }
                    for item in items.iter().skip(2) {
                        index_sexpr(item, false, defs, refs);
                    }
                }

                // (lambda/fn (params...) body...)
                Some("lambda") | Some("fn") => {
                    if let Some(params_expr) = items.get(1) {
                        if let SExprKind::List(params) = &params_expr.kind {
                            for p in params {
                                if let SExprKind::Atom(Atom::Symbol(name)) = &p.kind {
                                    let name_span = p.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                                    defs.push(SymbolDef {
                                        name: name.clone(),
                                        kind: SymbolDefKind::Lambda,
                                        name_span,
                                        form_span,
                                        detail: Some("param".to_string()),
                                    });
                                }
                            }
                        }
                    }
                    for item in items.iter().skip(2) {
                        index_sexpr(item, false, defs, refs);
                    }
                }

                // Other forms: head is a reference, recurse into args
                _ => {
                    // Head of a call is a reference
                    if let Some(head) = items.first() {
                        if let SExprKind::Atom(Atom::Symbol(name)) = &head.kind {
                            // Don't record kernel keywords as references
                            if !is_keyword(name) {
                                let span = head.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
                                refs.push(SymbolRef { name: name.clone(), span });
                            }
                        }
                    }
                    for item in items.iter().skip(1) {
                        index_sexpr(item, false, defs, refs);
                    }
                }
            }
        }

        SExprKind::Atom(Atom::Symbol(name)) if !in_quote => {
            // A bare symbol is a reference (unless it starts with ? which is a meta/hole)
            if !name.starts_with('?') && !is_keyword(name) && !is_constant(name) {
                refs.push(SymbolRef {
                    name: name.clone(),
                    span: form_span,
                });
            }
        }

        SExprKind::Quote(inner) | SExprKind::QuasiQuote(inner) => {
            // Inside quote: don't collect refs (metadata), but recurse shallowly
            index_sexpr(inner, true, defs, refs);
        }

        SExprKind::Unquote(inner) | SExprKind::UnquoteSplicing(inner) => {
            // Back to code context
            index_sexpr(inner, false, defs, refs);
        }

        // Quoted list, atoms in quote, numbers, strings — skip
        _ => {}
    }
}

fn is_keyword(s: &str) -> bool {
    matches!(s,
        "def" | "define" | "define-facet" | "touch" | "rule" | "sugar"
        | "let" | "begin" | "do" | "if" | "cond" | "match" | "case"
        | "lambda" | "fn" | "use" | "in" | "quote" | "quasiquote"
        | "unquote" | "unquote-splicing" | "cons" | "set!"
        | "assert" | "assert-coherent" | "check" | "verify"
        | "module" | "export" | "import" | "require"
        | "context" | "goal" | "coherence" | "as"
        | "grade" | "transport"
    )
}

fn is_constant(s: &str) -> bool {
    matches!(s, "true" | "false" | "nil" | "#t" | "#f")
}
