use edgelord_lsp::document::{
    ByteSpan, apply_content_changes, offset_to_position, position_to_offset,
    selection_chain_is_well_formed, top_level_symbols,
};
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent};

#[test]
fn utf16_position_offset_roundtrip_is_stable() {
    let text = "(def x \"a😀b\")\n(def y x)\n";

    let mut offsets = vec![0usize, text.len()];
    offsets.extend(text.char_indices().map(|(idx, _)| idx));
    offsets.sort_unstable();
    offsets.dedup();

    for offset in offsets {
        let pos = offset_to_position(text, offset);
        let roundtrip = position_to_offset(text, pos);
        assert_eq!(roundtrip, offset, "failed at offset {}", offset);
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
fn incremental_changes_respect_input_order() {
    let base = "(def)\n";
    let first_then_second = vec![
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 4), Position::new(0, 4))),
            range_length: None,
            text: " x".to_string(),
        },
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 4), Position::new(0, 4))),
            range_length: None,
            text: " y".to_string(),
        },
    ];
    let second_then_first = vec![
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 4), Position::new(0, 4))),
            range_length: None,
            text: " y".to_string(),
        },
        TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 4), Position::new(0, 4))),
            range_length: None,
            text: " x".to_string(),
        },
    ];

    let out_a = apply_content_changes(base, &first_then_second);
    let out_b = apply_content_changes(base, &second_then_first);
    assert_eq!(out_a, "(def y x)\n");
    assert_eq!(out_b, "(def x y)\n");
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
