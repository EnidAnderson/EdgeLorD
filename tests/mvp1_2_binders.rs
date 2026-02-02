use edgelord_lsp::document::{BindingKind, ParsedDocument};

#[test]
fn let_binder_is_visible_inside_body() {
    let text = "(def f (let x v ?g))\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let goal = parsed.goals.first().expect("goal must exist");

    let names = goal.context.iter().map(|b| b.name.as_str()).collect::<Vec<_>>();
    assert!(names.contains(&"x"));
}

#[test]
fn nested_lets_shadow_outer_binders() {
    let text = "(def f (let x v (let x w ?g)))\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let goal = parsed.goals.first().expect("goal must exist");

    let xs = goal
        .context
        .iter()
        .filter(|b| b.name == "x")
        .collect::<Vec<_>>();
    assert_eq!(xs.len(), 1);
    assert_eq!(xs[0].kind, BindingKind::Let);
}

#[test]
fn local_let_bindings_precede_top_level_bindings() {
    let text = "(touch g)\n(def h g)\n(def f (let x v ?q))\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let goal = parsed
        .goals
        .iter()
        .find(|g| g.name.as_deref() == Some("q"))
        .expect("goal must exist");

    assert!(goal.context.len() >= 3);
    assert_eq!(goal.context[0].name, "x");
    assert_eq!(goal.context[0].kind, BindingKind::Let);
    assert_eq!(goal.context[1].name, "g");
    assert_eq!(goal.context[2].name, "h");
}

#[test]
fn let_list_binders_are_left_to_right() {
    let text = "(def f (let (x y z) v ?q))\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let goal = parsed.goals.first().expect("goal must exist");

    let names = goal
        .context
        .iter()
        .take(3)
        .map(|b| b.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["x", "y", "z"]);
}
