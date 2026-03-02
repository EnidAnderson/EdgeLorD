//! SE4: Tactic applicability index — given a rule, find all applicable goals.
//!
//! Inverts SC0's question: instead of "what rules for this goal?", we ask
//! "what goals for this rule?"
//!
//! **INV D-***: grouped and sorted deterministically.

use std::collections::BTreeMap;

use comrade_lisp::core::CompiledRule;
use comrade_lisp::proof_state::{MorMetaId, ProofState};
use serde::{Deserialize, Serialize};

use crate::tactics::pattern_find::{find_rule_occurrences, OccurrenceScope, PatternOccurrence};
use crate::tactics::speculative::try_rule_on_goal;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativePreview {
    pub solves_goal: bool,
    pub changes_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicabilitySite {
    pub occurrence: PatternOccurrence,
    pub speculative_preview: Option<SpeculativePreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticApplicabilityReport {
    pub rule_name: String,
    pub total_sites: usize,
    pub by_goal: BTreeMap<String, Vec<ApplicabilitySite>>, // key: goal_id as string
    pub would_solve: Vec<String>,          // goal_id strings
    pub would_make_progress: Vec<String>,
}

// ─── Engine ──────────────────────────────────────────────────────────────────

/// Compute applicability report for `rule` across `proof_state`.
///
/// **INV D-***: occurrences from SE0 are sorted; BTreeMap groups; would_solve sorted.
pub fn tactic_applicability(
    rule: &CompiledRule,
    proof_state: &ProofState,
    speculative_fuel: usize,
) -> TacticApplicabilityReport {
    let scope = OccurrenceScope::AllGoals;
    let occurrences = find_rule_occurrences(proof_state, rule, &scope);

    let mut by_goal: BTreeMap<String, Vec<ApplicabilitySite>> = BTreeMap::new();
    let mut would_solve: Vec<String> = Vec::new();
    let mut would_progress: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for occ in &occurrences {
        let goal_key = occ.goal_id.to_string();
        let goal_id = MorMetaId(occ.goal_id);

        let preview = if checked < speculative_fuel {
            checked += 1;
            try_rule_on_goal(proof_state, goal_id, rule).map(|spec| {
                if spec.solved && !would_solve.contains(&goal_key) {
                    would_solve.push(goal_key.clone());
                }
                if spec.changed && !would_progress.contains(&goal_key) {
                    would_progress.push(goal_key.clone());
                }
                SpeculativePreview {
                    solves_goal: spec.solved,
                    changes_status: spec.changed,
                }
            })
        } else {
            None
        };

        by_goal
            .entry(goal_key)
            .or_default()
            .push(ApplicabilitySite {
                occurrence: occ.clone(),
                speculative_preview: preview,
            });
    }

    // INV D-*: sort
    would_solve.sort_unstable();
    would_solve.dedup();
    would_progress.sort_unstable();
    would_progress.dedup();

    TacticApplicabilityReport {
        rule_name: rule.name.clone(),
        total_sites: occurrences.len(),
        by_goal,
        would_solve,
        would_make_progress: would_progress,
    }
}

/// Hover badge text for a rule: "N applicable site(s) in current proof"
pub fn hover_badge(report: &TacticApplicabilityReport) -> String {
    if report.total_sites == 0 {
        "*No applicable sites*".to_string()
    } else {
        let solve_note = if !report.would_solve.is_empty() {
            format!(", would solve {} goal(s)", report.would_solve.len())
        } else {
            String::new()
        };
        format!(
            "**{} applicable site(s)** in current proof{}",
            report.total_sites, solve_note
        )
    }
}
