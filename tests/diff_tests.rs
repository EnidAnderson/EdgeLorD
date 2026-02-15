use edgelord_lsp::diff::engine::compute_diff;
use edgelord_lsp::goals_panel::GoalChangeKind;
use comrade_lisp::proof_state::{
    ProofState, GoalState, GoalStatus, LocalContext, ObjExpr, ElaborationTrace, MorMetaId, 
    MorType, HoleOwner, MetaSubst
};
use comrade_lisp::diagnostics::projection::GoalsPanelIndex;
use comrade_lisp::diagnostics::DiagnosticContext;
use source_span::Span;
use comrade_lisp::proof_state;

#[test]
fn test_status_change_diff() {
    let obj0 = ObjExpr::Meta(proof_state::ObjMetaId(0));
    let mor_type = MorType { src: obj0.clone(), dst: obj0 };
    let owner = HoleOwner::TopLevel { form_index: 0 };
    let empty_lc = LocalContext { entries: vec![], doctrine: None };
    let m1 = MorMetaId(1);

    let ps1 = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner: owner.clone(),
                ordinal: 0,
                span: Some(Span::new(0, 5)),
                local_context: empty_lc.clone(),
                expected_type: mor_type.clone(),
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

    let ctx = DiagnosticContext::new("test.ml".to_string(), "test.ml");
    let index1 = GoalsPanelIndex::new(&ps1, &ctx);

    let ps2 = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner: owner.clone(),
                ordinal: 0,
                span: Some(Span::new(0, 5)),
                local_context: empty_lc.clone(),
                expected_type: mor_type,
                status: GoalStatus::Solved(proof_state::MorExpr::Meta(MorMetaId(99))),
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
    let index2 = GoalsPanelIndex::new(&ps2, &ctx);

    let diff = compute_diff(&ps1, &index1, &ps2, &index2);
    let anchor = index1.meta_to_anchor.get(&m1).unwrap();
    let delta = diff.get(anchor).expect("Delta not found for goal");

    match &delta.changes[0] {
        GoalChangeKind::StatusChanged { old_status, new_status } => {
            assert_eq!(*old_status, edgelord_lsp::goals_panel::GoalStatus::Unsolved);
            assert_eq!(*new_status, edgelord_lsp::goals_panel::GoalStatus::SOLVED);
        }
        _ => panic!("Expected StatusChanged delta, got {:?}", delta.changes[0]),
    }
}

#[test]
fn test_blockers_change_diff() {
    let obj0 = ObjExpr::Meta(proof_state::ObjMetaId(0));
    let mor_type = MorType { src: obj0.clone(), dst: obj0 };
    let owner_m1 = HoleOwner::TopLevel { form_index: 0 };
    let owner_m2 = HoleOwner::Def("foo".to_string());
    let owner_m3 = HoleOwner::Def("bar".to_string());
    let empty_lc = LocalContext { entries: vec![], doctrine: None };
    let m1 = MorMetaId(1);
    let m2 = MorMetaId(2);
    let m3 = MorMetaId(3);

    let ps1 = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner: owner_m1.clone(),
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
                owner: owner_m2,
                ordinal: 0,
                span: Some(Span::new(10, 15)),
                local_context: empty_lc.clone(),
                expected_type: mor_type.clone(),
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

    let ctx = DiagnosticContext::new("test.ml".to_string(), "test.ml");
    let index1 = GoalsPanelIndex::new(&ps1, &ctx);

    let ps2 = ProofState {
        goals: vec![
            GoalState {
                id: m1,
                name: "goal1".to_string(),
                owner: owner_m1,
                ordinal: 0,
                span: Some(Span::new(0, 5)),
                local_context: empty_lc.clone(),
                expected_type: mor_type.clone(),
                status: GoalStatus::Blocked { depends_on: vec![m3] },
                relevant_constraints: vec![],
            },
            GoalState {
                id: m3,
                name: "goal3".to_string(),
                owner: owner_m3,
                ordinal: 0,
                span: Some(Span::new(20, 25)),
                local_context: empty_lc.clone(),
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
    let index2 = GoalsPanelIndex::new(&ps2, &ctx);

    let diff = compute_diff(&ps1, &index1, &ps2, &index2);
    let anchor_m1 = index1.meta_to_anchor.get(&m1).unwrap();
    let delta = diff.get(anchor_m1).expect("Delta not found for goal 1");

    let mut blocker_changes = false;
    for change in &delta.changes {
        if let GoalChangeKind::BlockersChanged { added, removed } = change {
            assert!(added.len() == 1);
            assert!(removed.len() == 1);
            blocker_changes = true;
        }
    }
    assert!(blocker_changes, "Expected BlockersChanged delta");
    
    // Goal 2 was removed, Goal 3 was added
    let anchor_m2 = index1.meta_to_anchor.get(&m2).unwrap();
    let anchor_m3 = index2.meta_to_anchor.get(&m3).unwrap();
    
    assert!(matches!(diff.get(anchor_m2).unwrap().changes[0], GoalChangeKind::Removed));
    assert!(matches!(diff.get(anchor_m3).unwrap().changes[0], GoalChangeKind::Added));
}
