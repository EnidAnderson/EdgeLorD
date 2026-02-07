use edgelord_lsp::highlight::{compute_layer0_structural, tokens_to_lsp_data, SymbolRole};

#[test]
fn test_layer1_highlighting_valid() {
    let text = "(def my-id (x : TYPE) x)";
    
    let mut tokens = compute_layer0_structural(text);
    let lsp_tokens = tokens_to_lsp_data(text, &mut tokens);
    
    assert!(!lsp_tokens.is_empty());
    
    // Check "def" is KernelHead
    let def_token = tokens.iter().find(|(span, _)| &text[span.start..span.end] == "def");
    assert!(def_token.is_some(), "def token not found");
    assert_eq!(def_token.unwrap().1, SymbolRole::KernelHead);
    
    // Check "my-id" is Definition
    let name_token = tokens.iter().find(|(span, _)| &text[span.start..span.end] == "my-id");
    assert!(name_token.is_some(), "my-id token not found");
    assert_eq!(name_token.unwrap().1, SymbolRole::Definition);
}

#[test]
fn test_layer0_fallback_invalid() {
    let text = "(def broken"; // Missing closing paren
    
    let tokens = compute_layer0_structural(text);
    
    // Fallback scanner only detects parens, strings, comments, numbers.
    // So we expect "(" to be found.
    let paren = tokens.iter().find(|(span, _)| &text[span.start..span.end] == "(");
    assert!(paren.is_some(), "Open paren not found in fallback");
    assert_eq!(paren.unwrap().1, SymbolRole::Structural);
    
    // "def" is skipped by simple fallback scanner
    let def = tokens.iter().find(|(span, _)| &text[span.start..span.end] == "def");
    assert!(def.is_none(), "Unexpectedly found 'def' in fallback (scanner shouldn't detect identifiers yet)");
}
