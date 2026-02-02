use edgelord_lsp::document::{ByteSpan, ParsedDocument};

#[test]
fn inlay_hints_are_deterministic() {
    let text = "(def f ?x)\n(def g (hole h))\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let range = ByteSpan::new(0, text.len());

    let a = parsed.goal_inlay_hints_in_range(range);
    let b = parsed.goal_inlay_hints_in_range(range);

    assert_eq!(a, b);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].label, "?x : unknown");
    assert_eq!(a[1].label, "hole h : unknown");
}

#[test]
fn inlay_hints_respect_requested_range() {
    let text = "(def f ?x)\n(def g ?y)\n";
    let parsed = ParsedDocument::parse(text.to_string());

    let first_line = ByteSpan::new(0, text.find('\n').unwrap_or(text.len()));
    let hints = parsed.goal_inlay_hints_in_range(first_line);

    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].label, "?x : unknown");
}

#[test]
fn inlay_hints_order_is_stable_with_same_offset() {
    let text = "(def f ?x ?y)\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let hints = parsed.goal_inlay_hints_in_range(ByteSpan::new(0, text.len()));

    assert_eq!(hints.len(), 2);
    assert!(hints[0].offset <= hints[1].offset);
}
