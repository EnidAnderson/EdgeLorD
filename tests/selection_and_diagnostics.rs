use edgelord_lsp::document::{ByteSpan, ParsedDocument, selection_chain_is_well_formed};

#[test]
fn selection_chain_expands_atom_list_form_root() {
    let text = "(begin (def x y))\n".to_string();
    let parsed = ParsedDocument::parse(text.clone());
    assert!(parsed.diagnostics.is_empty());

    let offset = text.find('x').expect("x must exist");
    let chain = parsed.selection_chain_for_offset(offset);

    assert!(
        chain.len() >= 4,
        "expected at least atom/list/form/root chain"
    );
    assert!(selection_chain_is_well_formed(&chain));
    assert_eq!(chain[0], ByteSpan::new(offset, offset + 1));
    assert_eq!(chain.last().copied(), Some(ByteSpan::new(0, text.len())));
}

#[test]
fn parse_error_produces_stable_diagnostic() {
    let text = "(begin (def x y)".to_string();
    let parsed_a = ParsedDocument::parse(text.clone());
    let parsed_b = ParsedDocument::parse(text);

    assert_eq!(parsed_a.diagnostics.len(), 1);
    assert_eq!(parsed_a.diagnostics.len(), parsed_b.diagnostics.len());
    assert_eq!(
        parsed_a.diagnostics[0].message,
        parsed_b.diagnostics[0].message
    );
    assert_eq!(parsed_a.diagnostics[0].span, parsed_b.diagnostics[0].span);
}
