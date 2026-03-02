//! SD3: Doctrine-parameterized proof strategies.
//!
//! Strategies are ordered lists of phases; each phase tries a set of rules
//! sequentially before moving to the next.
//!
//! **INV D-***: phases applied in declaration order; rules within a phase in declaration order.

use crate::tactics::rule_index::RuleIndex;
use crate::tactics::speculative::try_rule_on_goal;
use comrade_lisp::proof_state::{GoalStatus, MorMetaId, ProofState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ─── Data model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseFallback {
    TryWitness(Vec<String>),
    AutoSearch(usize),
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPhase {
    pub name: String,
    pub description: String,
    pub rule_names: Vec<String>,
    pub max_applications: usize,
    pub fallback: Option<PhaseFallback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub name: String,
    pub description: String,
    pub phases: Vec<StrategyPhase>,
    pub doctrine_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyApplyResult {
    pub applied_phases: Vec<String>,
    pub applied_rules: Vec<(String, String)>, // (phase, rule)
    pub solved: bool,
    pub partial: bool,
    pub message: String,
}

// ─── Built-in strategies ─────────────────────────────────────────────────────

/// Motivic homotopy theory standard strategy.
pub fn motivic_standard_strategy() -> Strategy {
    Strategy {
        name: "motivic-standard".to_string(),
        description: "Standard motivic homotopy strategy".to_string(),
        doctrine_context: Some("motivic".to_string()),
        phases: vec![
            StrategyPhase {
                name: "stabilize".to_string(),
                description: "Get into the stable range".to_string(),
                rule_names: vec![
                    "stability-loop-susp".to_string(),
                    "stability-susp-loop".to_string(),
                    "susp-tensor".to_string(),
                    "grade-suspension".to_string(),
                    "grade-loop".to_string(),
                ],
                max_applications: 10,
                fallback: None,
            },
            StrategyPhase {
                name: "normalize-twists".to_string(),
                description: "Normalize Tate twist composition".to_string(),
                rule_names: vec![
                    "tate-compose".to_string(),
                    "tate-unit".to_string(),
                    "tate-grade".to_string(),
                ],
                max_applications: 10,
                fallback: None,
            },
            StrategyPhase {
                name: "pullback".to_string(),
                description: "Reduce via pullback/base-change".to_string(),
                rule_names: vec![
                    "pull-push-triangle".to_string(),
                    "push-pull-triangle".to_string(),
                    "pullback-compose".to_string(),
                    "pullback-id".to_string(),
                ],
                max_applications: 10,
                fallback: Some(PhaseFallback::TryWitness(vec![
                    "bc-check".to_string(),
                    "frobenius-check".to_string(),
                ])),
            },
            StrategyPhase {
                name: "descent".to_string(),
                description: "Verify descent".to_string(),
                rule_names: vec![
                    "descent-cover-glue".to_string(),
                    "a1-locality".to_string(),
                    "nisnevich-compose".to_string(),
                    "nisnevich-id".to_string(),
                ],
                max_applications: 10,
                fallback: Some(PhaseFallback::TryWitness(vec![
                    "descent-check".to_string(),
                ])),
            },
        ],
    }
}

/// Differential/CDC standard strategy.
pub fn differential_standard_strategy() -> Strategy {
    Strategy {
        name: "differential-standard".to_string(),
        description: "Standard differential cohomology strategy".to_string(),
        doctrine_context: Some("differential".to_string()),
        phases: vec![
            StrategyPhase {
                name: "normalize-D".to_string(),
                description: "Push differential inside".to_string(),
                rule_names: vec![
                    "diff-compose".to_string(),
                    "diff-tensor".to_string(),
                    "diff-id".to_string(),
                ],
                max_applications: 10,
                fallback: None,
            },
            StrategyPhase {
                name: "simplify-plug".to_string(),
                description: "Simplify plugging".to_string(),
                rule_names: vec![
                    "plug-zero".to_string(),
                    "plug-compose".to_string(),
                ],
                max_applications: 10,
                fallback: None,
            },
            StrategyPhase {
                name: "algebra".to_string(),
                description: "Apply algebraic laws".to_string(),
                rule_names: vec![
                    "compose-id-left".to_string(),
                    "compose-id-right".to_string(),
                    "compose-assoc".to_string(),
                ],
                max_applications: 20,
                fallback: Some(PhaseFallback::AutoSearch(30)),
            },
        ],
    }
}

// ─── Strategy registry ───────────────────────────────────────────────────────

/// Built-in strategy registry. **INV D-***: BTreeMap.
pub fn built_in_strategies() -> BTreeMap<String, Strategy> {
    let mut map = BTreeMap::new();
    let motivic = motivic_standard_strategy();
    let diff = differential_standard_strategy();
    map.insert(motivic.name.clone(), motivic);
    map.insert(diff.name.clone(), diff);
    map
}

/// Execute a strategy against the proof state.
/// Returns a summary of what was applied at which phases.
pub fn execute_strategy(
    strategy: &Strategy,
    proof: &ProofState,
    goal_id: MorMetaId,
    index: &RuleIndex,
) -> StrategyApplyResult {
    let mut applied_phases = Vec::new();
    let mut applied_rules: Vec<(String, String)> = Vec::new();
    let mut solved = false;

    'phase: for phase in &strategy.phases {
        let mut phase_applied = false;
        for rule_name in &phase.rule_names {
            // Find rule in index by name — check no-doctrine rules first, then doctrine-specific
            let no_doctrine_rules = index.rules_for_doctrine(None);
            let doctrine_rules = strategy.doctrine_context.as_deref()
                .map(|d| index.rules_for_doctrine(Some(d)))
                .unwrap_or_default();

            let rule = no_doctrine_rules.into_iter()
                .chain(doctrine_rules.into_iter())
                .find(|r| &r.name == rule_name);

            let rule = match rule { Some(r) => r, None => continue };

            for _ in 0..phase.max_applications {
                let spec = try_rule_on_goal(proof, goal_id, rule);
                if let Some(s) = spec {
                    if s.solved {
                        applied_rules.push((phase.name.clone(), rule.name.clone()));
                        applied_phases.push(phase.name.clone());
                        solved = true;
                        break 'phase;
                    }
                    if s.changed {
                        applied_rules.push((phase.name.clone(), rule.name.clone()));
                        phase_applied = true;
                        break;
                    }
                }
                break;
            }
        }
        if phase_applied {
            applied_phases.push(phase.name.clone());
        }
    }

    let message = if solved {
        format!("Goal solved in {} phase(s)", applied_phases.len())
    } else if !applied_rules.is_empty() {
        format!(
            "Applied {} rules across {} phase(s), goal not yet solved",
            applied_rules.len(),
            applied_phases.len()
        )
    } else {
        "No rules applicable in any phase".to_string()
    };

    let partial = !applied_rules.is_empty();
    StrategyApplyResult {
        applied_phases,
        applied_rules,
        solved,
        partial,
        message,
    }
}
