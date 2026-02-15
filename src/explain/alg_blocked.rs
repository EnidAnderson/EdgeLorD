use crate::explain::builder::ExplainBuilder;
use crate::explain::view::{ExplanationView, ExplanationKind, ExplainLimits};
use comrade_lisp::proof_state::{ProofState, GoalStatus, MorMetaId};
use comrade_lisp::diagnostics::projection::GoalsPanelIndex;
use std::collections::{BTreeSet, HashMap};
use source_span::Span;
use comrade_lisp::proof_state;

/// Explain why a goal is blocked: show linear blocker chains for highest impact metas.
pub fn explain_why_blocked(
    goal_anchor_id: &str,
    proof_state: &ProofState,
    index: &GoalsPanelIndex,
    limits: ExplainLimits,
) -> ExplanationView {
    let mut builder = ExplainBuilder::new(limits);
    
    // Resolve anchor to actual goal
    let goal_opt = index.resolve_anchor(goal_anchor_id)
        .and_then(|(meta_id, _span)| {
            proof_state.goals.iter().find(|g| g.id == meta_id)
        });
    
    let goal = match goal_opt {
        Some(g) => g,
        None => {
            builder.set_root(goal_anchor_id.to_string(), ExplanationKind::Blocked, format!("Goal not found: {}", goal_anchor_id), None);
            return builder.build();
        }
    };
    
    // 1. Get direct blockers
    let direct_blockers = match &goal.status {
        GoalStatus::Blocked { depends_on } => depends_on,
        _ => {
            builder.set_root(
                goal_anchor_id.to_string(), 
                ExplanationKind::Blocked, 
                format!("Goal ?{} is not blocked (Status: {:?})", goal.name, goal.status),
                goal.span
            );
            return builder.build();
        }
    };

    let root_idx = builder.set_root(
        goal_anchor_id.to_string(),
        ExplanationKind::Blocked,
        format!("Why blocked: ?{}", goal.name),
        goal.span
    );
    builder.add_metadata(root_idx, "blocker_count".to_string(), direct_blockers.len().to_string());

    // 2. Rank blockers by impact (dependent goals count)
    // We'll compute impact on-the-fly for this proof state
    let impact_map = compute_meta_impact(proof_state);
    
    let mut sorted_blockers: Vec<MorMetaId> = direct_blockers.iter().cloned().collect();
    sorted_blockers.sort_by(|a, b| {
        let impact_a = impact_map.get(a).unwrap_or(&0);
        let impact_b = impact_map.get(b).unwrap_or(&0);
        impact_b.cmp(impact_a).then(a.cmp(b)) // Descending impact, ascending ID
    });

    // 3. Produce up to k chains (default 3)
    let k = 3;
    for (i, &meta_id) in sorted_blockers.iter().take(k).enumerate() {
        let chain_root_id = format!("{}:chain:{}", goal_anchor_id, i);
        let impact = impact_map.get(&meta_id).unwrap_or(&0);
        
        let label = match find_goal_for_meta(proof_state, meta_id) {
            Some(g) => format!("Metavariable ?{} (Blocks {} goals)", g.name, impact),
            None => format!("Metavariable {:?} (Blocks {} goals)", meta_id, impact),
        };

        if let Some(mut current_idx) = builder.add_child(root_idx, chain_root_id, ExplanationKind::BlockerChain, label, find_span_for_meta(proof_state, meta_id)) {
            // Expand chain up to length 10
            let mut chain_len = 1;
            let mut current_meta = meta_id;
            let mut seen_in_chain = BTreeSet::new();
            seen_in_chain.insert(meta_id);

            while chain_len < 10 {
                // Find what blocks current_meta
                if let Some(next_blocker) = find_primary_blocker(proof_state, current_meta, &impact_map) {
                    if seen_in_chain.contains(&next_blocker) {
                        break; // Cycle detected
                    }
                    
                    let impact_next = impact_map.get(&next_blocker).unwrap_or(&0);
                    let label_next = match find_goal_for_meta(proof_state, next_blocker) {
                        Some(g) => format!("Metavariable ?{} (Impact: {})", g.name, impact_next),
                        None => format!("Metavariable {:?} (Impact: {})", next_blocker, impact_next),
                    };
                    
                    let next_id = format!("{}:step:{}", builder.get_node(current_idx).id, chain_len);
                    if let Some(next_idx) = builder.add_child(current_idx, next_id, ExplanationKind::MetaDependency, label_next, find_span_for_meta(proof_state, next_blocker)) {
                        current_idx = next_idx;
                        current_meta = next_blocker;
                        seen_in_chain.insert(next_blocker);
                        chain_len += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }

    builder.build()
}

fn compute_meta_impact(ps: &ProofState) -> HashMap<MorMetaId, usize> {
    let mut map = HashMap::new();
    for goal in &ps.goals {
        if let GoalStatus::Blocked { depends_on } = &goal.status {
            for &meta in depends_on {
                *map.entry(meta).or_insert(0) += 1;
            }
        }
    }
    map
}

fn find_goal_for_meta(ps: &ProofState, meta_id: MorMetaId) -> Option<&proof_state::GoalState> {
    ps.goals.iter().find(|g| g.id == meta_id)
}

fn find_span_for_meta(ps: &ProofState, meta_id: MorMetaId) -> Option<Span> {
    find_goal_for_meta(ps, meta_id).and_then(|g| g.span)
}

fn find_primary_blocker(ps: &ProofState, meta_id: MorMetaId, impact_map: &HashMap<MorMetaId, usize>) -> Option<MorMetaId> {
    let goal = find_goal_for_meta(ps, meta_id)?;
    if let GoalStatus::Blocked { depends_on } = &goal.status {
        // Canonical parent = highest impact, then smallest meta id
        let mut metas: Vec<MorMetaId> = depends_on.iter().cloned().collect();
        metas.sort_by(|a, b| {
            let i_a = impact_map.get(a).unwrap_or(&0);
            let i_b = impact_map.get(b).unwrap_or(&0);
            i_b.cmp(i_a).then(a.cmp(b))
        });
        metas.into_iter().next()
    } else {
        None
    }
}
