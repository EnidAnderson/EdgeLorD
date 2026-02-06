use std::collections::BTreeMap;
use std::sync::Arc;
use crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction};

/// A registry for all available tactics.
/// Uses BTreeMap to ensure deterministic iteration order.
pub struct TacticRegistry {
    tactics: BTreeMap<String, Arc<dyn Tactic>>,
}

impl TacticRegistry {
    pub fn new() -> Self {
        Self {
            tactics: BTreeMap::new(),
        }
    }

    /// Register a new tactic.
    pub fn register(&mut self, tactic: Arc<dyn Tactic>) {
        self.tactics.insert(tactic.id().to_string(), tactic);
    }

    /// Compute all applicable actions from all registered tactics.
    pub fn compute_all(&self, req: &TacticRequest) -> Vec<TacticAction> {
        let mut all_actions = Vec::new();

        for tactic in self.tactics.values() {
            match tactic.compute(req) {
                TacticResult::Actions(actions) => {
                    all_actions.extend(actions);
                }
                TacticResult::Truncated { actions, .. } => {
                    all_actions.extend(actions);
                }
                TacticResult::NotApplicable => {}
            }
        }

        // Deterministic sorting of final action set
        // Sort key: (safety, kind, title, action_id)
        all_actions.sort_by(|a, b| {
            (a.safety as u8).cmp(&(b.safety as u8))
                .then((a.kind as u8).cmp(&(b.kind as u8)))
                .then(a.title.cmp(&b.title))
                .then(a.action_id.cmp(&b.action_id))
        });

        all_actions
    }
}

impl Default for TacticRegistry {
    fn default() -> Self {
        Self::new()
    }
}
