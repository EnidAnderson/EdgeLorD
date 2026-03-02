//! SD2: Bounded breadth-first auto-tactic engine.
//!
//! Tries rule combinations in BFS order until a goal is solved, fuel is
//! exhausted, or the timeout fires.
//!
//! **INV D-***: deterministic — rules tried in name-sorted order.
//! **INV S-SPECULATIVE**: results are advisory; never certified.

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use comrade_lisp::core::CompiledRule;
use comrade_lisp::proof_state::{GoalStatus, MorMetaId, ProofState};
use serde::{Deserialize, Serialize};

use crate::tactics::rule_index::RuleIndex;
use crate::tactics::speculative::try_rule_on_goal;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoStep {
    pub rule_name: String,
    pub goal_id_u32: u32,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoTacticResult {
    Solved {
        steps: Vec<AutoStep>,
    },
    Partial {
        steps: Vec<AutoStep>,
        solved_count: usize,
        remaining_count: usize,
    },
    Stuck {
        reason: String,
        tried_count: usize,
    },
    Exhausted {
        best_steps: Vec<AutoStep>,
        best_solved: usize,
        tried_count: usize,
    },
}

#[derive(Debug, Clone)]
pub struct AutoLimits {
    pub fuel: usize,
    pub max_depth: usize,
    pub timeout_ms: u64,
    pub max_queue_size: usize,
}

impl Default for AutoLimits {
    fn default() -> Self {
        AutoLimits { fuel: 50, max_depth: 5, timeout_ms: 500, max_queue_size: 100 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoProgressSnapshot {
    pub tried: usize,
    pub fuel_remaining: usize,
    pub best_so_far: String,
}

// ─── BFS engine ───────────────────────────────────────────────────────────────

/// State tracked per BFS node.
#[derive(Clone)]
struct BfsNode {
    proof: ProofState,
    steps: Vec<AutoStep>,
    solved_count: usize,
}

/// Try to solve `goal_id` in `proof` using rules from `index`.
///
/// `progress_cb`: called every 10 fuel units with a progress snapshot.
/// **INV D-***: rules are iterated in name-sorted order.
pub fn auto_solve(
    proof: &ProofState,
    goal_id: MorMetaId,
    index: &RuleIndex,
    limits: AutoLimits,
    mut progress_cb: impl FnMut(AutoProgressSnapshot),
) -> AutoTacticResult {
    let timeout = Duration::from_millis(limits.timeout_ms);
    let start = Instant::now();

    let mut queue: VecDeque<BfsNode> = VecDeque::with_capacity(16);
    queue.push_back(BfsNode {
        proof: proof.clone(),
        steps: vec![],
        solved_count: 0,
    });

    let mut fuel = limits.fuel;
    let mut tried = 0usize;
    let mut best_solved = 0usize;
    let mut best_steps: Vec<AutoStep> = vec![];

    // Gather candidates sorted by name — INV D-*
    let mut all_rules: Vec<&CompiledRule> = index.rules_for_doctrine(None).into_iter().collect();
    let wildcard = index.rules_for_doctrine(Some("*"));
    all_rules.extend(wildcard);

    // Deduplicate by name — INV D-*
    let mut seen_names = BTreeSet::new();
    all_rules.retain(|r| seen_names.insert(r.name.clone()));
    all_rules.sort_by_key(|r| r.name.as_str());

    if all_rules.is_empty() {
        return AutoTacticResult::Stuck {
            reason: "No rules in index".to_string(),
            tried_count: 0,
        };
    }

    while let Some(node) = queue.pop_front() {
        if fuel == 0 || node.steps.len() >= limits.max_depth {
            break;
        }
        if start.elapsed() >= timeout {
            break;
        }
        if queue.len() >= limits.max_queue_size {
            break;
        }

        for rule in &all_rules {
            if fuel == 0 { break; }
            if start.elapsed() >= timeout { break; }

            let result = try_rule_on_goal(&node.proof, goal_id, rule);
            fuel -= 1;
            tried += 1;

            if tried % 10 == 0 {
                progress_cb(AutoProgressSnapshot {
                    tried,
                    fuel_remaining: fuel,
                    best_so_far: format!("Solved {} subgoal(s)", best_solved),
                });
            }

            if let Some(spec) = result {
                let new_step = AutoStep {
                    rule_name: rule.name.clone(),
                    goal_id_u32: goal_id.as_u32(),
                    confidence: if spec.solved { "exact".to_string() } else { "partial".to_string() },
                };
                let mut new_steps = node.steps.clone();
                new_steps.push(new_step);

                if spec.solved {
                    return AutoTacticResult::Solved { steps: new_steps };
                }

                if spec.changed && spec.consistent {
                    if node.solved_count > best_solved {
                        best_solved = node.solved_count;
                        best_steps = new_steps.clone();
                    }
                    if queue.len() < limits.max_queue_size {
                        queue.push_back(BfsNode {
                            proof: node.proof.clone(),
                            steps: new_steps,
                            solved_count: node.solved_count,
                        });
                    }
                }
            }
        }
    }

    if best_solved > 0 {
        let remaining = proof.goals.iter()
            .filter(|g| !matches!(g.status, GoalStatus::Solved(_)))
            .count();
        AutoTacticResult::Partial {
            steps: best_steps,
            solved_count: best_solved,
            remaining_count: remaining,
        }
    } else if tried == 0 {
        AutoTacticResult::Stuck { reason: "No applicable rules found".to_string(), tried_count: 0 }
    } else {
        AutoTacticResult::Exhausted { best_steps, best_solved, tried_count: tried }
    }
}
