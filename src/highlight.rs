

use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType, SemanticTokenModifier};
use crate::document::ByteSpan;
use new_surface_syntax::parser;
use new_surface_syntax::syntax::{SExpr, SExprKind, Atom};

pub struct HighlightCtx<'a> {
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolRole {
    KernelHead,
    Binder,
    Definition,
    RuleName,
    FacetSort,
    FacetOp,
    FacetConst,
    Meta,
    Structural, // Parens, dots
    String,
    Comment,
    Number,
    Keyword, // General keyword if not kernel head
    Unknown,
}

impl SymbolRole {
    pub fn to_lsp_type(&self) -> SemanticTokenType {
        match self {
            SymbolRole::KernelHead => SemanticTokenType::KEYWORD,
            SymbolRole::Binder => SemanticTokenType::VARIABLE,
            SymbolRole::Definition => SemanticTokenType::VARIABLE, // + DECLARATION
            SymbolRole::RuleName => SemanticTokenType::FUNCTION,
            SymbolRole::FacetSort => SemanticTokenType::TYPE,
            SymbolRole::FacetOp => SemanticTokenType::FUNCTION,
            SymbolRole::FacetConst => SemanticTokenType::VARIABLE, // + READONLY
            SymbolRole::Meta => SemanticTokenType::VARIABLE, // + MODIFICATION
            SymbolRole::Structural => SemanticTokenType::OPERATOR,
            SymbolRole::String => SemanticTokenType::STRING,
            SymbolRole::Comment => SemanticTokenType::COMMENT,
            SymbolRole::Number => SemanticTokenType::NUMBER,
            SymbolRole::Keyword => SemanticTokenType::KEYWORD,
            SymbolRole::Unknown => SemanticTokenType::VARIABLE,
        }
    }

    pub fn modifiers(&self) -> u32 {
        let mut bits = 0;
        match self {
            SymbolRole::Definition => bits |= 1 << 0, // DECLARATION (assumed index 0)
            SymbolRole::FacetConst => bits |= 1 << 1, // READONLY
            SymbolRole::Meta => bits |= 1 << 2, // MODIFICATION? No, standard ones only?
            // We need to match the Legend defined in server capabilities.
            // Standard modifiers: declaration, definition, readonly, static, deprecated, abstract, async, modification, documentation, defaultLibrary
            // Let's assume a standard legend order for now:
            // 0: declaration, 1: definition, 2: readonly, 3: static, 4: deprecated, 5: abstract, 6: async, 7: modification, 8: documentation, 9: defaultLibrary
            SymbolRole::KernelHead => bits |= 1 << 9, // defaultLibrary
            _ => {},
        }
        bits
    }
}

pub const LEGEND_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::TYPE,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::STRING,
    SemanticTokenType::COMMENT,
    SemanticTokenType::NUMBER,
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

/// Compute Layer 0 structural tokens.
pub fn compute_layer0_structural(text: &str) -> Vec<(ByteSpan, SymbolRole)> {
    let mut tokens = Vec::new();
    
    // 1. Try CST parse
    if let Ok(module) = parser::parse_module(text) {
        for form in &module.body {
            traverse_sexpr(form, &mut tokens);
        }
    } else {
        // Fallback: Lexical scan
        scan_fallback(text, &mut tokens);
    }
    
    tokens
}

fn traverse_sexpr(expr: &SExpr, out: &mut Vec<(ByteSpan, SymbolRole)>) {
    let span = expr.span.map(|s| ByteSpan::new(s.start, s.end)).unwrap_or(ByteSpan::new(0, 0));
    
    match &expr.kind {
        SExprKind::List(items) => {
            // Check for special forms (Layer 1)
            let head_str = if let Some(head) = items.first() {
                if let SExprKind::Atom(Atom::Symbol(s)) = &head.kind {
                    Some(s.as_str())
                } else {
                    None
                }
            } else {
                None
            };
            
            if let Some(head) = head_str {
                match head {
                    "def" => {
                        // (def name type value) or (def name value)
                        // Highlight 'def' as KernelHead
                        
                        // We can manually push KernelHead for items[0] if we want to override default behavior
                        // But traverse_sexpr recursively calls.
                        // Better: handle items manually here.
                        handle_special_form_head(&items[0], SymbolRole::KernelHead, out);
                        
                        if items.len() > 1 {
                            // Name is Definition
                            handle_special_form_head(&items[1], SymbolRole::Definition, out);
                        }
                        // Rest are normal
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, out);
                        }
                        return;
                    }
                    "touch" => {
                        // (touch name type) or (touch name)
                        handle_special_form_head(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            handle_special_form_head(&items[1], SymbolRole::Binder, out);
                        }
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, out);
                        }
                        return;
                    }
                    "rule" => {
                        // (rule name (params...) LHS RHS)
                        handle_special_form_head(&items[0], SymbolRole::KernelHead, out);
                        if items.len() > 1 {
                            handle_special_form_head(&items[1], SymbolRole::RuleName, out);
                        }
                        // TODO: Handle binders in param list?
                        for item in items.iter().skip(2) {
                            traverse_sexpr(item, out);
                        }
                        return;
                    }
                    "let" => {
                        handle_special_form_head(&items[0], SymbolRole::KernelHead, out);
                        // (let (binders...) body...)
                        // or (let name val body...)
                        for item in items.iter().skip(1) {
                            traverse_sexpr(item, out);
                        }
                        return;
                    }
                    "quote" | "quasiquote" | "unquote" | "unquote-splicing" | "begin" | "do" => {
                        handle_special_form_head(&items[0], SymbolRole::KernelHead, out);
                        for item in items.iter().skip(1) {
                            traverse_sexpr(item, out);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            
            // Standard list traversal
            for (i, item) in items.iter().enumerate() {
                if i == 0 && head_str.is_some() {
                     // Ensure the head is handled. If not special, it might be a function call (FacetOp?)
                     // For Layer 1, if it's not a keyword, we treat as Unknown or Function?
                     // Spec says "List head position". 
                     // We'll let recursive call handle it as Atom::Symbol -> Unknown
                     // But we could optimistically mark as Function?
                     // Let's stick to traversing.
                     traverse_sexpr(item, out);
                } else {
                    traverse_sexpr(item, out);
                }
            }
        }
        SExprKind::Atom(atom) => {
            let role = match atom {
                Atom::String(_) => SymbolRole::String,
                Atom::Integer(_) => SymbolRole::Number,
                Atom::Symbol(s) => {
                    if s.starts_with('?') {
                        SymbolRole::Meta
                    } else if s.starts_with('"') { 
                        SymbolRole::String 
                    } else if s.chars().all(|c| c.is_digit(10)) {
                        SymbolRole::Number
                    } else {
                        SymbolRole::Unknown
                    }
                }

            };
            out.push((span, role));
        }
        SExprKind::Quote(inner) | SExprKind::QuasiQuote(inner) | SExprKind::Unquote(inner) | SExprKind::UnquoteSplicing(inner) => {
            out.push((span, SymbolRole::KernelHead)); // The quote char itself is a kernel head syntax
            traverse_sexpr(inner, out);
        }
    }
}

fn handle_special_form_head(expr: &SExpr, role: SymbolRole, out: &mut Vec<(ByteSpan, SymbolRole)>) {
    // Helper to force a role on an expression (usually an atom)
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

