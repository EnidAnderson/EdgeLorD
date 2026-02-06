use edgelord_lsp::tactics::{
    view::{Selection, TacticLimits, TacticRequest, TacticResult, Tactic},
    registry::TacticRegistry,
    stdlib::quickfix::AddTouchTactic,
};
use edgelord_lsp::edgelord_pretty_ctx::EdgeLordPrettyCtx;
use edgelord_lsp::document::ParsedDocument;
use new_surface_syntax::proof_state::{ProofState, MetaSubst, ElaborationTrace};
use tower_lsp::lsp_types::{Range, Position, Url};
use std::sync::Arc;

#[test]
fn test_registry_compute_all() {
    let mut registry = TacticRegistry::new();
    registry.register(Arc::new(AddTouchTactic));

    // Setup mock request
    let doc = ParsedDocument::parse("(hole test)".to_string());
    let ps = ProofState {
        goals: vec![],
        constraints: vec![],
        subst: MetaSubst::new(),
        trace: ElaborationTrace::new(),
        conflicts: vec![],
        solver_error: None,
        cycles: vec![],
    };
    
    let uri = Url::parse("file:///test.ml").unwrap();
    let registry_pretty = new_surface_syntax::diagnostics::pretty::PrinterRegistry::new_with_defaults();
    let files = new_surface_syntax::diagnostics::DiagnosticContext::new("test.ml".to_string(), "");
    let pretty_ctx = EdgeLordPrettyCtx::new(
        &registry_pretty,
        new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
        new_surface_syntax::diagnostics::pretty::PrettyLimits::hover_default(),
        &ps,
        &files,
        &uri,
    );

    let req = TacticRequest {
        ctx: &pretty_ctx,
        proof: &ps,
        doc: &doc,
        index: None,
        selection: Selection {
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
        },
        limits: TacticLimits::default(),
    };

    let actions = registry.compute_all(&req);
    // Should find the hole and propose add_touch
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Add (touch test) before this goal");
}

#[test]
fn test_add_touch_tactic_skips_if_exists() {
    let tactic = AddTouchTactic;
    let text = "(touch test)\n(hole test)".to_string();
    let doc = ParsedDocument::parse(text.clone());
    let ps = ProofState {
        goals: vec![],
        constraints: vec![],
        subst: MetaSubst::new(),
        trace: ElaborationTrace::new(),
        conflicts: vec![],
        solver_error: None,
        cycles: vec![],
    };
    
    let uri = Url::parse("file:///test.ml").unwrap();
    let registry_pretty = new_surface_syntax::diagnostics::pretty::PrinterRegistry::new_with_defaults();
    let files = new_surface_syntax::diagnostics::DiagnosticContext::new("test.ml".to_string(), "");
    let pretty_ctx = EdgeLordPrettyCtx::new(
        &registry_pretty,
        new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
        new_surface_syntax::diagnostics::pretty::PrettyLimits::hover_default(),
        &ps,
        &files,
        &uri,
    );

    let req = TacticRequest {
        ctx: &pretty_ctx,
        proof: &ps,
        doc: &doc,
        index: None,
        selection: Selection {
            range: Range::new(Position::new(1, 0), Position::new(1, 10)),
        },
        limits: TacticLimits::default(),
    };

    let result = tactic.compute(&req);
    match result {
        TacticResult::NotApplicable => {}
        _ => panic!("Expected NotApplicable because touch already exists"),
    }
}

#[test]
fn test_focus_goal_tactic() {
    let tactic = edgelord_lsp::tactics::stdlib::goaldirected::FocusGoalTactic;
    let text = "(hole test)".to_string();
    let doc = ParsedDocument::parse(text.clone());
    let ps = ProofState {
        goals: vec![],
        constraints: vec![],
        subst: MetaSubst::new(),
        trace: ElaborationTrace::new(),
        conflicts: vec![],
        solver_error: None,
        cycles: vec![],
    };
    
    let uri = Url::parse("file:///test.ml").unwrap();
    let registry_pretty = new_surface_syntax::diagnostics::pretty::PrinterRegistry::new_with_defaults();
    let files = new_surface_syntax::diagnostics::DiagnosticContext::new("test.ml".to_string(), "");
    let pretty_ctx = EdgeLordPrettyCtx::new(
        &registry_pretty,
        new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
        new_surface_syntax::diagnostics::pretty::PrettyLimits::hover_default(),
        &ps,
        &files,
        &uri,
    );

    let req = TacticRequest {
        ctx: &pretty_ctx,
        proof: &ps,
        doc: &doc,
        index: None,
        selection: Selection {
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
        },
        limits: TacticLimits::default(),
    };

    let result = tactic.compute(&req);
    match result {
        TacticResult::Actions(actions) => {
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].title, "Focus on this goal");
        }
        _ => panic!("Expected Actions"),
    }
}
