use crate::explain::builder::ExplainBuilder;
use crate::explain::view::{ExplanationView, ExplanationKind, ExplainLimits};
use new_surface_syntax::proof_state::{ProofState, GoalStatus};
use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;

/// Explain what a goal is: its context, target, and direct evidence.
pub fn explain_goal(
    goal_anchor_id: &str,
    proof_state: &ProofState,
    index: &GoalsPanelIndex,
    limits: ExplainLimits,
) -> ExplanationView {
    let mut builder = ExplainBuilder::new(limits);
    
    // Resolve anchor
    let goal_opt = index.resolve_anchor(goal_anchor_id)
        .and_then(|(meta_id, _span)| {
            proof_state.goals.iter().find(|g| g.id == meta_id)
        });
    
    let goal = match goal_opt {
        Some(g) => g,
        None => {
            builder.set_root(goal_anchor_id.to_string(), ExplanationKind::GoalEmission, format!("Goal not found: {}", goal_anchor_id), None);
            return builder.build();
        }
    };
    
    // Root = GoalEmission
    let root_idx = builder.set_root(
        goal_anchor_id.to_string(),
        ExplanationKind::GoalEmission,
        format!("Goal: ?{}", goal.name),
        goal.span
    );

    // 1. Target Summary (Shallow)
    // In a real implementation we'd use PrettyCtx here
    let target_label = format!("Target: {:?} → {:?}", goal.expected_type.src, goal.expected_type.dst);
    builder.add_child(root_idx, format!("{}:target", goal_anchor_id), ExplanationKind::GoalEmission, target_label, None);

    // 2. Direct Constraints
    let mut sorted_constraints = goal.relevant_constraints.clone();
    sorted_constraints.sort_by_key(|c| format!("{:?}", c)); // Deterministic

    for (i, constraint) in sorted_constraints.iter().enumerate() {
        let label = format!("Constraint: {:?}", constraint);
        let id = format!("{}:constraint:{}", goal_anchor_id, i);
        builder.add_child(root_idx, id, ExplanationKind::Constraint, label, None);
    }

    // 3. Direct Meta Dependencies
    if let GoalStatus::Blocked { depends_on } = &goal.status {
        let mut sorted_metas: Vec<_> = depends_on.iter().cloned().collect();
        sorted_metas.sort();
        
        for (i, &meta) in sorted_metas.iter().enumerate() {
            let label = format!("Dependency: metavariable {:?}", meta);
            let id = format!("{}:dep:{}", goal_anchor_id, i);
            builder.add_child(root_idx, id, ExplanationKind::MetaDependency, label, find_span_for_meta(proof_state, meta));
        }
    }

    builder.build()
}

fn find_span_for_meta(ps: &ProofState, meta_id: new_surface_syntax::proof_state::MorMetaId) -> Option<source_span::Span> {
    ps.goals.iter().find(|g| g.id == meta_id).and_then(|g| g.span)
}
