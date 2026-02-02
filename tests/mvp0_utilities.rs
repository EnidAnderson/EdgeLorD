use edgelord_lsp::document::{
    ByteSpan, apply_content_changes, offset_to_position, position_to_offset,
    selection_chain_is_well_formed, top_level_symbols,
};
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent};

#[test]
fn utf16_position_offset_roundtrip_is_stable() {
    let text = "(def x \"a😀b\")\n(def y x)\n";

    let points = [
        Position::new(0, 0),
        Position::new(0, 9),
        Position::new(0, 10),
        Position::new(0, 12),
        Position::new(1, 3),
    ];

    for pos in points {
        let offset = position_to_offset(text, pos);
        let roundtrip = offset_to_position(text, offset);
        assert_eq!(roundtrip, pos, "failed at {:?}", pos);
    }
}

#[test]
fn incremental_changes_apply_deterministically() {
    let base = "(begin (def x y))\n";
    let changes = vec![
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 12), Position::new(0, 13))),
            range_length: None,
            text: "z".to_string(),
        },
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 0), Position::new(0, 0))),
            range_length: None,
            text: ";; header\n".to_string(),
        },
    ];

    let out_a = apply_content_changes(base, &changes);
    let out_b = apply_content_changes(base, &changes);
    assert_eq!(out_a, out_b);
    assert_eq!(out_a, ";; header\n(begin (def z y))\n");
}

#[test]
fn top_level_symbols_order_is_stable() {
    let text = "(touch a)\n(def a b)\n(rule x y (meta tag))\n";

    let a = top_level_symbols(text);
    let b = top_level_symbols(text);
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);

    assert_eq!(a[0].1, ByteSpan::new(0, 9));
}

#[test]
fn selection_chain_validation_rejects_non_nested_spans() {
    let invalid = [ByteSpan::new(3, 7), ByteSpan::new(0, 5)];
    assert!(!selection_chain_is_well_formed(&invalid));
}
