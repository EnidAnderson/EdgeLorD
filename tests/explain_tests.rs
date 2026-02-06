use edgelord_lsp::explain::builder::ExplainBuilder;
use edgelord_lsp::explain::view::{ExplanationKind, ExplainLimits, validate_span};
use edgelord_lsp::explain::alg_goal::explain_goal;
use edgelord_lsp::explain::alg_blocked::explain_why_blocked;
use edgelord_lsp::explain::alg_inconsistent::explain_why_inconsistent;
use new_surface_syntax::proof_state::{
    ProofState, GoalState, GoalStatus, LocalContext, ObjExpr, ElaborationTrace, MorMetaId, 
    MorType, HoleOwner, MetaSubst
};
use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;
use new_surface_syntax::diagnostics::DiagnosticContext;
use source_span::Span;

#[test]
fn test_builder_determinism() {
    let limits = ExplainLimits::default();
    
    let run1 = {
        let mut b = ExplainBuilder::new(limits.clone());
        let r = b.set_root("root".to_string(), ExplanationKind::GoalEmission, "Root".to_string(), None);
        b.add_child(r, "id-b".to_string(), ExplanationKind::Constraint, "B".to_string(), None);
        b.add_child(r, "id-a".to_string(), ExplanationKind::Constraint, "A".to_string(), None);
        b.build()
    };
    
    let run2 = {
        let mut b = ExplainBuilder::new(limits);
        let r = b.set_root("root".to_string(), ExplanationKind::GoalEmission, "Root".to_string(), None);
        b.add_child(r, "id-a".to_string(), ExplanationKind::Constraint, "A".to_string(), None);
        b.add_child(r, "id-b".to_string(), ExplanationKind::Constraint, "B".to_string(), None);
        b.build()
    };
    
    assert_eq!(run1.root.children[0].id, "id-a");
    assert_eq!(run1.root.children[1].id, "id-b");
    assert_eq!(run1, run2);
}

#[test]
fn test_jump_target_validity() {
    let text_len = 50;
    let s1 = Span::new(10, 20);
    assert_eq!(validate_span(s1, text_len), Some(s1));
    let s2 = Span::new(10, 60);
    assert_eq!(validate_span(s2, text_len), None);
    let s3 = Span::new(20, 10);
    assert_eq!(validate_span(s3, text_len), None);
}

#[test]
fn test_explain_why_blocked_snapshot() {
    let obj0 = ObjExpr::Meta(new_surface_syntax::proof_state::ObjMetaId(0));
    let mor_type = MorType { src: obj0.clone(), dst: obj0 };
    let owner = HoleOwner::TopLevel { form_index: 0 };
    let empty_lc = LocalContext { entries: vec![], doctrine: None };

    let m1 = MorMetaId(1);
    let m2 = MorMetaId(2);
    let m3 = MorMetaId(3);

    let ps = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner: owner.clone(),
                ordinal: 0,
                span: Some(Span::new(0, 5)),
                local_context: empty_lc.clone(),
                expected_type: mor_type.clone(),
                status: GoalStatus::Blocked { depends_on: vec![m2] },
                relevant_constraints: vec![],
            },
            GoalState {
                id: m2,
                name: "goal2".to_string(),
                owner: owner.clone(),
                ordinal: 1,
                span: Some(Span::new(10, 15)),
                local_context: empty_lc.clone(),
                expected_type: mor_type.clone(),
                status: GoalStatus::Blocked { depends_on: vec![m3] },
                relevant_constraints: vec![],
            },
            GoalState {
                id: m3,
                name: "goal3".to_string(),
                owner: owner,
                ordinal: 2,
                span: Some(Span::new(20, 25)),
                local_context: empty_lc,
                expected_type: mor_type,
                status: GoalStatus::Unsolved,
                relevant_constraints: vec![],
            },
        ],
        constraints: vec![],
        subst: MetaSubst::new(),
        trace: ElaborationTrace::new(),
        conflicts: vec![],
        solver_error: None,
        cycles: vec![],
    };
    
    let diag_ctx = DiagnosticContext::new("test_file_id".to_string(), "test.maclane");
    let index = GoalsPanelIndex::new(&ps, &diag_ctx);
    
    let g1_anchor = index.meta_to_anchor.get(&m1).expect("m1 should have an anchor").clone();
    
    let view = explain_why_blocked(&g1_anchor, &ps, &index, ExplainLimits::default());
    
    assert_eq!(view.root.kind, ExplanationKind::Blocked);
    let chains = &view.root.children;
    assert!(!chains.is_empty());
    
    let chain0 = &chains[0];
    assert_eq!(chain0.kind, ExplanationKind::BlockerChain);
    // Impact ranking test: goal2 should be in the chain
    assert!(chain0.label.contains("?goal2"));
}

#[test]
fn test_explain_goal_snapshot() {
    let obj0 = ObjExpr::Meta(new_surface_syntax::proof_state::ObjMetaId(0));
    let mor_type = MorType { src: obj0.clone(), dst: obj0 };
    let owner = HoleOwner::TopLevel { form_index: 0 };
    let empty_lc = LocalContext { entries: vec![], doctrine: None };
    let m1 = MorMetaId(1);

    let ps = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner,
                ordinal: 0,
                span: Some(Span::new(0, 5)),
                local_context: empty_lc,
                expected_type: mor_type,
                status: GoalStatus::Unsolved,
                relevant_constraints: vec![],
            },
        ],
        constraints: vec![],
        subst: MetaSubst::new(),
        trace: ElaborationTrace::new(),
        conflicts: vec![],
        solver_error: None,
        cycles: vec![],
    };
    
    let diag_ctx = DiagnosticContext::new("test_id".to_string(), "test.maclane");
    let index = GoalsPanelIndex::new(&ps, &diag_ctx);
    let anchor = index.meta_to_anchor.get(&m1).unwrap();
    
    let view = explain_goal(anchor, &ps, &index, ExplainLimits::default());
    assert_eq!(view.root.kind, ExplanationKind::GoalEmission);
    assert!(view.root.label.contains("?goal1"));
}

#[test]
fn test_explain_why_inconsistent_snapshot() {
    let obj0 = ObjExpr::Meta(new_surface_syntax::proof_state::ObjMetaId(0));
    let mor_type = MorType { src: obj0.clone(), dst: obj0 };
    let owner = HoleOwner::TopLevel { form_index: 0 };
    let empty_lc = LocalContext { entries: vec![], doctrine: None };
    let m1 = MorMetaId(1);

    let ps = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner,
                ordinal: 0,
                span: Some(Span::new(0, 5)),
                local_context: empty_lc,
                expected_type: mor_type,
                status: GoalStatus::Inconsistent { conflicts: vec![] }, // Mocked empty conflicts
                relevant_constraints: vec![],
            },
        ],
        constraints: vec![],
        subst: MetaSubst::new(),
        trace: ElaborationTrace::new(),
        conflicts: vec![new_surface_syntax::proof_state::ConstraintId(0)],
        solver_error: None,
        cycles: vec![],
    };
    
    let diag_ctx = DiagnosticContext::new("test_id".to_string(), "test.maclane");
    let index = GoalsPanelIndex::new(&ps, &diag_ctx);
    let anchor = index.meta_to_anchor.get(&m1).unwrap();
    
    let view = explain_why_inconsistent(anchor, &ps, &index, ExplainLimits::default());
    assert_eq!(view.root.kind, ExplanationKind::Conflict);
}
