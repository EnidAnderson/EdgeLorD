use std::collections::{BTreeMap, BTreeSet};
use new_surface_syntax::proof_state::{ProofState, MorMetaId, GoalStatus as KernelGoalStatus};
use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;
use crate::goals_panel::{GoalDelta, GoalChangeKind, GoalStatus};

/// Computes the semantic difference between two proof states.
pub fn compute_diff(
    old_ps: &ProofState,
    old_index: &GoalsPanelIndex,
    new_ps: &ProofState,
    new_index: &GoalsPanelIndex,
) -> BTreeMap<String, GoalDelta> {
    let mut deltas = BTreeMap::new();

    let old_map: BTreeMap<String, MorMetaId> = old_index.meta_to_anchor.iter()
        .map(|(k, v)| (v.clone(), *k)).collect();
    let new_map: BTreeMap<String, MorMetaId> = new_index.meta_to_anchor.iter()
        .map(|(k, v)| (v.clone(), *k)).collect();

    let old_anchors: BTreeSet<String> = old_map.keys().cloned().collect();
    let new_anchors: BTreeSet<String> = new_map.keys().cloned().collect();

    // 1. Added goals
    for anchor in new_anchors.difference(&old_anchors) {
        deltas.insert(anchor.clone(), GoalDelta {
            changes: vec![GoalChangeKind::Added],
        });
    }

    // 2. Removed goals (optional to track, but good for completeness)
    for anchor in old_anchors.difference(&new_anchors) {
        deltas.insert(anchor.clone(), GoalDelta {
            changes: vec![GoalChangeKind::Removed],
        });
    }

    // 3. Modified goals
    for anchor in old_anchors.intersection(&new_anchors) {
        let old_id = old_map.get(anchor).unwrap();
        let new_id = new_map.get(anchor).unwrap();

        let old_goal = old_ps.goals.iter().find(|g| g.id == *old_id);
        let new_goal = new_ps.goals.iter().find(|g| g.id == *new_id);

        if let (Some(old_g), Some(new_g)) = (old_goal, new_goal) {
            let mut changes = Vec::new();

            // Status change
            let old_status = map_status(&old_g.status);
            let new_status = map_status(&new_g.status);
            if old_status != new_status {
                changes.push(GoalChangeKind::StatusChanged { 
                    old_status, 
                    new_status 
                });
            }

            // Blockers change
            let old_blockers = get_blockers(&old_g.status, old_index);
            let new_blockers = get_blockers(&new_g.status, new_index);
            if old_blockers != new_blockers {
                let added: Vec<_> = new_blockers.difference(&old_blockers).cloned().collect();
                let removed: Vec<_> = old_blockers.difference(&new_blockers).cloned().collect();
                changes.push(GoalChangeKind::BlockersChanged { added, removed });
            }

            // TODO: Title and Context changes (requires PrettyCtx)

            if !changes.is_empty() {
                deltas.insert(anchor.clone(), GoalDelta { changes });
            }
        }
    }

    deltas
}

fn map_status(status: &KernelGoalStatus) -> GoalStatus {
    match status {
        KernelGoalStatus::Unsolved => GoalStatus::Unsolved,
        KernelGoalStatus::Blocked { .. } => GoalStatus::Blocked,
        KernelGoalStatus::Solved(_) => GoalStatus::SOLVED,
        KernelGoalStatus::Inconsistent { .. } => GoalStatus::Error,
    }
}

fn get_blockers(status: &KernelGoalStatus, index: &GoalsPanelIndex) -> BTreeSet<String> {
    if let KernelGoalStatus::Blocked { depends_on } = status {
        depends_on.iter().filter_map(|id| index.meta_to_anchor.get(id).cloned()).collect()
    } else {
        BTreeSet::new()
    }
}
