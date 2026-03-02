use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType, SemanticTokenModifier};
use crate::document::ByteSpan;
use comrade_lisp::parser;
use comrade_lisp::syntax::{SExpr, SExprKind, Atom};

pub struct HighlightCtx<'a> {
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolRole {
    /// Core language forms: def, rule, touch, let, begin, sugar, use, in, lambda, ...
    KernelHead,
    /// Binding occurrences in touch, let, lambda parameter lists
    Binder,
    /// Name introduced by (def name ...)
    Definition,
    /// Name of a rewrite rule: (rule name ...)
    RuleName,
    /// Type/sort names: doctrine names, module paths
    FacetSort,
    /// Function/operation in call position (head of a non-special list)
    FacetOp,
    /// Constants: true, false, nil
    FacetConst,
    /// Pattern/meta variables: ?x, ?*xs
    Meta,
    /// Parentheses, brackets
    Structural,
    /// String literals
    String,
    /// ; line comments
    Comment,
    /// Integer literals
    Number,
    /// Built-in operations: prelude/motivic dispatched operations
    Keyword,
    /// Unclassified identifiers
    Unknown,
    /// Module path segments in (use Module::Path ...)
    Namespace,
    /// Quoted metadata: '(meta ...) blocks
    Property,
}

impl SymbolRole {
    pub fn to_lsp_type(&self) -> SemanticTokenType {
        match self {
            SymbolRole::KernelHead => SemanticTokenType::KEYWORD,
            SymbolRole::Binder => SemanticTokenType::VARIABLE,
            SymbolRole::Definition => SemanticTokenType::VARIABLE,
            SymbolRole::RuleName => SemanticTokenType::FUNCTION,
            SymbolRole::FacetSort => SemanticTokenType::TYPE,
            SymbolRole::FacetOp => SemanticTokenType::FUNCTION,
            SymbolRole::FacetConst => SemanticTokenType::ENUM_MEMBER,
            SymbolRole::Meta => SemanticTokenType::PARAMETER,
            SymbolRole::Structural => SemanticTokenType::OPERATOR,
            SymbolRole::String => SemanticTokenType::STRING,
            SymbolRole::Comment => SemanticTokenType::COMMENT,
            SymbolRole::Number => SemanticTokenType::NUMBER,
            SymbolRole::Keyword => SemanticTokenType::KEYWORD,
            SymbolRole::Unknown => SemanticTokenType::VARIABLE,
            SymbolRole::Namespace => SemanticTokenType::NAMESPACE,
            SymbolRole::Property => SemanticTokenType::PROPERTY,
        }
    }

    pub fn modifiers(&self) -> u32 {
        // Modifier bit indices match LEGEND_TOKEN_MODIFIERS order:
        // 0: declaration, 1: definition, 2: readonly, 3: static,
        // 4: deprecated, 5: abstract, 6: async, 7: modification,
        // 8: documentation, 9: defaultLibrary
        match self {
            SymbolRole::Definition => 1 << 0,  // declaration
            SymbolRole::FacetConst => 1 << 2,  // readonly
            SymbolRole::Meta => 1 << 7,        // modification
            SymbolRole::KernelHead => 1 << 9,  // defaultLibrary
            SymbolRole::Keyword => 1 << 9,     // defaultLibrary
            _ => 0,
        }
    }
}

pub const LEGEND_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,       // 0
    SemanticTokenType::VARIABLE,      // 1
    SemanticTokenType::FUNCTION,      // 2
    SemanticTokenType::TYPE,          // 3
    SemanticTokenType::OPERATOR,      // 4
    SemanticTokenType::STRING,        // 5
    SemanticTokenType::COMMENT,       // 6
    SemanticTokenType::NUMBER,        // 7
    SemanticTokenType::ENUM_MEMBER,   // 8  (for constants: true/false/nil)
    SemanticTokenType::PARAMETER,     // 9  (for meta/pattern variables: ?x)
    SemanticTokenType::NAMESPACE,     // 10 (for module paths)
    SemanticTokenType::PROPERTY,      // 11 (for metadata keys)
];

pub const LEGEND_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,
    SemanticTokenModifier::DEFINITION,
    SemanticTokenModifier::READONLY,
    SemanticTokenModifier::STATIC,
    SemanticTokenModifier::DEPRECATED,
    SemanticTokenModifier::ABSTRACT,
    SemanticTokenModifier::ASYNC,
    SemanticTokenModifier::MODIFICATION,
    SemanticTokenModifier::DOCUMENTATION,
    SemanticTokenModifier::DEFAULT_LIBRARY,
];

/// Encodes internal tokens to LSP delta format (Absolute -> Relative).
/// Must be sorted by Position.
pub fn tokens_to_lsp_data(text: &str, tokens: &mut [(ByteSpan, SymbolRole)]) -> Vec<SemanticToken> {
    // 1. Sort tokens structurally
    tokens.sort_by(|a, b| a.0.start.cmp(&b.0.start));

    let mut data = Vec::new();
    let mut last_line = 0;
    let mut last_start_char = 0;

    for (span, role) in tokens {
        // Convert byte span to line/col
        // We need 'position_at' logic.
        // Re-implementing simplified version or importing from document?
        // Reuse crate::document::offset_to_position
        let start_pos = crate::document::offset_to_position(text, span.start);
        
        // Ensure Span length in UTF-16?
        // Semantic Token length is in *characters* (UTF-16 usually).
        // We need length of the slice.
        let slice = &text[span.start..span.end];
        let len_utf16 = slice.chars().map(|c| c.len_utf16()).sum::<usize>() as u32;
        
        if len_utf16 == 0 { continue; }

        let line = start_pos.line;
        let col = start_pos.character;

        let delta_line = line - last_line;
        let delta_start = if delta_line == 0 {
            col - last_start_char
        } else {
            col
        };

        // Lookup token type index
        let token_type_idx = LEGEND_TOKEN_TYPES.iter().position(|t| *t == role.to_lsp_type()).unwrap_or(0) as u32;
        let token_modifiers_bitset = role.modifiers();

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: len_utf16,
            token_type: token_type_idx,
            token_modifiers_bitset,
        });

        last_line = line;
        last_start_char = col;
    }
    data
}

// ── Keyword classification ──────────────────────────────────────────

/// Core language forms that receive `KernelHead` highlighting.
fn is_kernel_keyword(s: &str) -> bool {
    matches!(s,
        "def" | "touch" | "rule" | "sugar" | "let" | "begin" | "do"
        | "quote" | "quasiquote" | "unquote" | "unquote-splicing"
        | "use" | "in" | "lambda" | "fn" | "if" | "cond" | "match" | "case"
        | "cons" | "nil" | "set!" | "define" | "define-facet"
        | "assert" | "assert-coherent" | "check" | "verify"
        | "module" | "export" | "import" | "require"
        | "context" | "goal" | "coherence"
    )
}

/// Built-in constants that receive `FacetConst` highlighting.
fn is_builtin_constant(s: &str) -> bool {
    matches!(s, "true" | "false" | "nil" | "#t" | "#f")
}

/// Prelude / motivic operations that receive `Keyword` highlighting
/// when used in call position.
fn is_prelude_operation(s: &str) -> bool {
    // H-series traces
    s.ends_with("-trace")
    || s.starts_with("check-")
    || s.starts_with("assert-")
    || s.starts_with("verify-")
    // Grade / prelude namespace operations
    || s.starts_with("grade/")
    || s.starts_with("prelude/")
    || s.starts_with("adjoint/")
    || s.starts_with("facet/")
    || s.starts_with("doctrine/")
    // Specific well-known operations
    || matches!(s,
        "normalize" | "transport" | "coherent?" | "classify-reduction"
        | "compose-recipes" | "check-diagram-coherence"
        | "tensor" | "oplus" | "hom" | "eval" | "curry"
        | "picard" | "tate" | "spectrum" | "d" | "pullback"
    )
}

// ── Comment pre-pass ────────────────────────────────────────────────

/// Scan text for `;` line comments before CST parsing (the lexer discards them).
fn scan_comments(text: &str, out: &mut Vec<(ByteSpan, SymbolRole)>) {
    let mut in_string = false;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_string => {
                in_string = true;
                i += 1;
            }
            b'"' if in_string => {
                in_string = false;
                i += 1;
            }
            b'\\' if in_string => {
                i += 2; // skip escaped char
            }
            b';' if !in_string => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                out.push((ByteSpan::new(start, i), SymbolRole::Comment));
            }
            _ => {
                i += 1;
            }
        }
    }
}

// ── Main entry point ────────────────────────────────────────────────

/// Compute structural + semantic tokens for a `.maclane` document.
///
/// Layer 0: structural (parens, strings, numbers, comments)
/// Layer 1: form-aware (def/rule/touch/sugar/use/in + call heads + meta variables)
pub fn compute_layer0_structural(text: &str) -> Vec<(ByteSpan, SymbolRole)> {
    let mut tokens = Vec::new();

    // Pre-pass: comments (lexer discards them, so we must scan separately)
    scan_comments(text, &mut tokens);

    // CST-based traversal for everything else
    if let Ok(module) = parser::parse_module(text) {
        for form in &module.body {
            traverse_sexpr(form, false, &mut tokens);
        }
    } else {
        // Fallback: lexical scan for broken files
        scan_fallback(text, &mut tokens);
    }

    tokens
}

// ── CST traversal ───────────────────────────────────────────────────

/// Recursively traverse an s-expression, emitting highlight tokens.
///
/// `in_quote`: true when inside a `'(...)` or `` `(...)` `` — metadata context.
fn traverse_sexpr(expr: &SExpr, in_quote: bool, out: &mut Vec<(ByteSpan, SymbolRole)>) {
    let span = expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));

    match &expr.kind {
        SExprKind::List(items) => {
            // Extract head symbol if present
            let head_str = items.first().and_then(|head| {
                if let SExprKind::Atom(Atom::Symbol(s)) = &head.kind {
                    Some(s.as_str())
                } else {
                    None
                }
            });

            // Inside quoted metadata: highlight keys as Property
            if in_quote {
                if let Some(head) = head_str {
                    if matches!(head, "meta" | "priority" | "kind" | "proof" | "law"
                        | "provenance" | "axiom" | "theorem" | "classes") {
                        emit(items.first().unwrap(), SymbolRole::Property, out);
                        for item in items.iter().skip(1) {
                            traverse_sexpr(item, true, out);
                        }
                        return;
                    }
                }
                // Generic quoted list — traverse children in quote context
                for item in items {
                    traverse_sexpr(item, true, out);
                }
                return;
            }

            if let Some(head) = head_str {
                match head {
                    // ── (def name ...) ──
                    "def" | "define" | "define-facet" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            emit(&items[1], SymbolRole::Definition, out);
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── (touch name [type]) ──
                    "touch" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            emit(&items[1], SymbolRole::Binder, out);
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── (rule name LHS RHS ['(meta ...)]) ──
                    "rule" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            emit(&items[1], SymbolRole::RuleName, out);
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── (sugar name pattern template) ──
                    "sugar" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            emit(&items[1], SymbolRole::RuleName, out);
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── (use Module::Path symbol [as alias]) ──
                    "use" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        // Module path segments get Namespace highlighting
                        if items.len() > 1 {
                            emit(&items[1], SymbolRole::Namespace, out);
                        }
                        // Imported symbol
                        if items.len() > 2 {
                            emit(&items[2], SymbolRole::Definition, out);
                        }
                        // "as" keyword
                        if items.len() > 3 {
                            if let SExprKind::Atom(Atom::Symbol(s)) = &items[3].kind {
                                if s == "as" {
                                    emit(&items[3], SymbolRole::KernelHead, out);
                                    if items.len() > 4 {
                                        emit(&items[4], SymbolRole::Definition, out);
                                    }
                                }
                            }
                        }
                        return;
                    }

                    // ── (in doctrine-name body...) ──
                    "in" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            emit(&items[1], SymbolRole::FacetSort, out);
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── (lambda/fn (params...) body...) ──
                    "lambda" | "fn" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        // Highlight parameter list entries as binders
                        if items.len() > 1 {
                            if let SExprKind::List(params) = &items[1].kind {
                                for p in params {
                                    emit(p, SymbolRole::Binder, out);
                                }
                            } else {
                                emit(&items[1], SymbolRole::Binder, out);
                            }
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── (let (bindings...) body) ──
                    "let" => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        // If second item is a list, treat odd entries as binders
                        if items.len() > 1 {
                            if let SExprKind::List(bindings) = &items[1].kind {
                                for (i, b) in bindings.iter().enumerate() {
                                    if i % 2 == 0 {
                                        emit(b, SymbolRole::Binder, out);
                                    } else {
                                        traverse_sexpr(b, false, out);
                                    }
                                }
                            } else {
                                // (let name val body...) — name as binder
                                emit(&items[1], SymbolRole::Binder, out);
                            }
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── Other kernel keywords: begin, do, if, cond, match, ... ──
                    kw if is_kernel_keyword(kw) => {
                        emit(&items[0], SymbolRole::KernelHead, out);
                        for item in items.iter().skip(1) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── Prelude / motivic operations in call position ──
                    op if is_prelude_operation(op) => {
                        emit(&items[0], SymbolRole::Keyword, out);
                        for item in items.iter().skip(1) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }

                    // ── Generic function call: (f args...) ──
                    _ => {
                        emit(&items[0], SymbolRole::FacetOp, out);
                        for item in items.iter().skip(1) {
                            traverse_sexpr(item, false, out);
                        }
                        return;
                    }
                }
            }

            // No head symbol (e.g. nested list, empty list) — traverse children
            for item in items {
                traverse_sexpr(item, in_quote, out);
            }
        }

        SExprKind::Atom(atom) => {
            let role = if in_quote {
                // Inside quoted form: atoms are metadata values
                match atom {
                    Atom::String(_) => SymbolRole::String,
                    Atom::Integer(_) => SymbolRole::Number,
                    Atom::Symbol(s) => {
                        if is_metadata_key(s) {
                            SymbolRole::Property
                        } else {
                            SymbolRole::String // treat quoted symbols as string-like
                        }
                    }
                }
            } else {
                match atom {
                    Atom::String(_) => SymbolRole::String,
                    Atom::Integer(_) => SymbolRole::Number,
                    Atom::Symbol(s) => classify_symbol(s),
                }
            };
            out.push((span, role));
        }

        SExprKind::Quote(inner) | SExprKind::QuasiQuote(inner) => {
            // Enter quoted/metadata context
            traverse_sexpr(inner, true, out);
        }

        SExprKind::Unquote(inner) | SExprKind::UnquoteSplicing(inner) => {
            // Unquote exits the quoted context back to code
            traverse_sexpr(inner, false, out);
        }
    }
}

/// Classify a bare symbol by its shape.
fn classify_symbol(s: &str) -> SymbolRole {
    if s.starts_with('?') {
        SymbolRole::Meta           // pattern variable: ?x, ?*xs
    } else if is_builtin_constant(s) {
        SymbolRole::FacetConst     // true, false, nil
    } else if s.contains("::") {
        SymbolRole::Namespace      // Module::Path references
    } else if s.starts_with('#') {
        SymbolRole::KernelHead     // #![ambient_doctrine ...]
    } else {
        SymbolRole::Unknown
    }
}

/// Check if a symbol is a recognized metadata key (inside quoted forms).
fn is_metadata_key(s: &str) -> bool {
    matches!(s,
        "meta" | "priority" | "kind" | "proof" | "law" | "provenance"
        | "axiom" | "theorem" | "lemma" | "corollary" | "classes"
        | "type" | "doctrine" | "origin" | "certified" | "structural"
    )
}

/// Emit a token with the given role for an expression.
fn emit(expr: &SExpr, role: SymbolRole, out: &mut Vec<(ByteSpan, SymbolRole)>) {
    let span = expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
    out.push((span, role));
}

/// Simple fallback scanner for Layer 0 when parser fails.
fn scan_fallback(text: &str, out: &mut Vec<(ByteSpan, SymbolRole)>) {
    let mut chars = text.char_indices().peekable();
    
    while let Some((idx, c)) = chars.next() {
        match c {
            '(' | ')' | '[' | ']' | '{' | '}' => {
                out.push((ByteSpan::new(idx, idx + 1), SymbolRole::Structural));
            }
            '"' => {
                // String literal
                let start = idx;
                let mut end = idx + 1;
                while let Some((next_idx, next_c)) = chars.next() {
                    end = next_idx + 1;
                    if next_c == '"' {
                        break;
                    }
                    if next_c == '\\' {
                        // Skip escaped char
                        let _ = chars.next(); 
                    }
                }
                out.push((ByteSpan::new(start, end), SymbolRole::String));
            }
            ';' => {
                // Comment (to end of line)
                let start = idx;
                let mut end = idx + 1;
                while let Some((next_idx, next_c)) = chars.peek() {
                    if *next_c == '\n' {
                        break;
                    }
                    end = *next_idx + 1;
                    chars.next();
                }
                out.push((ByteSpan::new(start, end), SymbolRole::Comment));
            }
            _ if c.is_digit(10) => {
                // Number (simplified)
                let start = idx;
                let mut end = idx + 1;
                while let Some((next_idx, next_c)) = chars.peek() {
                    if !next_c.is_digit(10) {
                        break;
                    }
                    end = *next_idx + 1;
                    chars.next();
                }
                out.push((ByteSpan::new(start, end), SymbolRole::Number));
            }
            _ => {}
        }
    }
}

