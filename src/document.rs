use std::cmp::Ordering;

use new_surface_syntax::{
    ParseError, parser,
    syntax::{Atom, SExpr, SExprKind},
};
use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub name: Option<String>,
    pub span: ByteSpan,
    pub context: Vec<String>,
    pub target: String,
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
    let span = ByteSpan::new(expr.span.start, expr.span.end);
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
        | ParseError::IllegalSplice { span } => ByteSpan::new(span.start, span.end),
        ParseError::UnexpectedEof { .. } => ByteSpan::new(text_len, text_len),
        ParseError::OldParseError(_) => ByteSpan::new(0, 0),
    }
}

fn extract_goals(forms: &[SExpr]) -> Vec<Goal> {
    let mut goals = Vec::new();
    let mut top_level_context = Vec::new();

    for form in forms {
        collect_holes_in_expr(form, &top_level_context, &mut goals);
        collect_top_level_bindings(form, &mut top_level_context);
    }

    goals
}

fn collect_holes_in_expr(expr: &SExpr, context: &[String], out: &mut Vec<Goal>) {
    match &expr.kind {
        SExprKind::Atom(Atom::Symbol(symbol)) if symbol.starts_with('?') => {
            let name = symbol.strip_prefix('?').map(|s| s.to_string());
            out.push(Goal {
                goal_id: goal_id(expr.span.start, expr.span.end, name.as_deref()),
                name,
                span: ByteSpan::new(expr.span.start, expr.span.end),
                context: context.to_vec(),
                target: "unknown".to_string(),
            });
        }
        SExprKind::List(items) => {
            if let Some(name) = hole_form_name(items) {
                out.push(Goal {
                    goal_id: goal_id(expr.span.start, expr.span.end, Some(name.as_str())),
                    name: Some(name),
                    span: ByteSpan::new(expr.span.start, expr.span.end),
                    context: context.to_vec(),
                    target: "unknown".to_string(),
                });
            }
            for item in items {
                collect_holes_in_expr(item, context, out);
            }
        }
        SExprKind::Quote(inner)
        | SExprKind::QuasiQuote(inner)
        | SExprKind::Unquote(inner)
        | SExprKind::UnquoteSplicing(inner) => collect_holes_in_expr(inner, context, out),
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

fn collect_top_level_bindings(form: &SExpr, context: &mut Vec<String>) {
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
    if !context.contains(name) {
        context.push(name.clone());
    }
}

fn goal_id(start: usize, end: usize, name: Option<&str>) -> String {
    let n = name.unwrap_or("anon");
    format!("goal-{start}-{end}-{n}")
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
        let span = ByteSpan::new(form.span.start, form.span.end);
        let label = match &form.kind {
            SExprKind::List(items) => match items.first() {
                Some(head) => match &head.kind {
                    SExprKind::Atom(atom) => format!("{:?}", atom),
                    _ => "list".to_string(),
                },
                None => "list".to_string(),
            },
            SExprKind::Atom(atom) => format!("{:?}", atom),
            SExprKind::Quote(_) => "quote".to_string(),
            SExprKind::QuasiQuote(_) => "quasiquote".to_string(),
            SExprKind::Unquote(_) => "unquote".to_string(),
            SExprKind::UnquoteSplicing(_) => "unquote-splicing".to_string(),
        };
        out.push((label, span));
    }
    out
}
