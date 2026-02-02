use edgelord_lsp::document::ParsedDocument;

#[test]
fn finds_unsolved_goals_with_stable_ids() {
    let text = "(touch a)\n(def f ?goal1)\n(def g (hole h2))\n";

    let parsed_a = ParsedDocument::parse(text.to_string());
    let parsed_b = ParsedDocument::parse(text.to_string());

    assert!(parsed_a.diagnostics.is_empty());
    assert_eq!(parsed_a.goals.len(), 2);
    assert_eq!(parsed_a.goals, parsed_b.goals);

    assert_eq!(parsed_a.goals[0].name.as_deref(), Some("goal1"));
    assert_eq!(parsed_a.goals[1].name.as_deref(), Some("h2"));
}

#[test]
fn goal_context_includes_prior_top_level_bindings() {
    let text = "(touch x)\n(def y x)\n(def p ?hole)\n";
    let parsed = ParsedDocument::parse(text.to_string());

    let goal = parsed
        .goals
        .iter()
        .find(|g| g.name.as_deref() == Some("hole"))
        .expect("goal should exist");

    let names = goal
        .context
        .iter()
        .map(|b| b.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"x"));
    assert!(names.contains(&"y"));
}

#[test]
fn goal_lookup_by_offset_works() {
    let text = "(def p ?h)\n";
    let parsed = ParsedDocument::parse(text.to_string());
    let offset = text.find("?h").expect("hole marker must exist");

    let goal = parsed.goal_at_offset(offset).expect("goal at offset must exist");
    assert_eq!(goal.name.as_deref(), Some("h"));
}
