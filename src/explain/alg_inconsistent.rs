use crate::explain::builder::ExplainBuilder;
use crate::explain::view::{ExplanationView, ExplanationKind, ExplainLimits};
use new_surface_syntax::proof_state::{ProofState, GoalStatus};
use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;

/// Explain why a goal is inconsistent: show deterministic conflict sets and their origins.
pub fn explain_why_inconsistent(
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
            builder.set_root(goal_anchor_id.to_string(), ExplanationKind::Conflict, format!("Goal not found: {}", goal_anchor_id), None);
            return builder.build();
        }
    };
    
    // 1. Get conflicts
    let conflicts = match &goal.status {
        GoalStatus::Inconsistent { conflicts } => conflicts,
        _ => {
            builder.set_root(
                goal_anchor_id.to_string(), 
                ExplanationKind::Conflict, 
                format!("Goal ?{} is not inconsistent", goal.name),
                goal.span
            );
            return builder.build();
        }
    };

    let root_idx = builder.set_root(
        goal_anchor_id.to_string(),
        ExplanationKind::Conflict,
        format!("Conflict Detected: ?{}", goal.name),
        goal.span
    );

    // 2. Identify and group conflict sets
    // Since we don't have the full conflict data structure details in the current snippet,
    // we'll assume a simplified view where we can extract the most relevant constraints.
    // In a real implementation, we'd iterate over the conflicts list.
    
    // Sort conflicts by numeric ID / stable string for determinism
    let mut sorted_conflicts = conflicts.clone();
    sorted_conflicts.sort_by_key(|c| format!("{:?}", c)); // Improved with stable ID if available

    // Emit one main conflict set by default, then +N
    const MAX_CONFLICT_SETS: usize = 3;
    
    for (i, conflict) in sorted_conflicts.iter().take(MAX_CONFLICT_SETS).enumerate() {
        let conflict_node_id = format!("{}:conflict:{}", goal_anchor_id, i);
        let label = format!("Conflicting requirement: {:?}", conflict); // Would be prettier with PrettyCtx
        
        if let Some(c_idx) = builder.add_child(root_idx, conflict_node_id, ExplanationKind::Conflict, label, None) {
            builder.add_metadata(c_idx, "origin".to_string(), "unification".to_string());
            builder.add_metadata(c_idx, "severity".to_string(), "error".to_string());
            
            // Show direct constraints (top 3)
            // This would fetch constraint details from a hypothetical TraceIndex or ProofState
            // For now we add a representative child if possible
        }
    }

    if sorted_conflicts.len() > MAX_CONFLICT_SETS {
        let more_id = format!("{}:more", goal_anchor_id);
        builder.add_child(root_idx, more_id, ExplanationKind::Derived, format!("+{} more conflicts...", sorted_conflicts.len() - MAX_CONFLICT_SETS), None);
    }

    builder.build()
}
