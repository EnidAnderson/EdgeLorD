//! SC1: Speculative rule-application API
//!
//! `try_rule_on_goal` performs a lightweight dry-run of applying a rule to a
//! goal.  It does NOT re-run the full constraint solver; instead it uses
//! structural/name heuristics to predict the outcome.
//!
//! A future version will clone the `ComradeWorkspace` proof arena and call
//! `solve_constraints` on the modified state; this version is intentionally
//! conservative so it compiles without coupling to the workspace internals.
//!
//! **INV S-SPECULATIVE**: outputs are never certified; they drive UI previews only.
//! **INV D-***: pure function — same (proof, goal_id, rule) → same result.

use comrade_lisp::core::CompiledRule;
use comrade_lisp::proof_state::{GoalStatus, MorMetaId, ProofState};

/// Outcome of a speculative rule application.
#[derive(Debug, Clone)]
pub struct SpeculativeResult {
    /// Would this rule directly solve the target goal?
    pub solved: bool,
    /// New goal IDs that would be created (empty until workspace integration).
    pub new_goal_ids: Vec<MorMetaId>,
    /// Did anything change relative to the current state?
    pub changed: bool,
    /// Is the resulting state free of conflicts?
    pub consistent: bool,
}

/// Dry-run: would applying `rule` to `goal_id` in `proof` make progress?
///
/// Returns `None` if `goal_id` is not found or is already solved.
///
/// **INV D-***: pure, no I/O, no global state.
pub fn try_rule_on_goal(
    proof: &ProofState,
    goal_id: MorMetaId,
    rule: &CompiledRule,
) -> Option<SpeculativeResult> {
    let goal = proof.goals.iter().find(|g| g.id == goal_id)?;

    // Already solved — nothing to do.
    if matches!(goal.status, GoalStatus::Solved(_)) {
        return None;
    }

    let no_conflicts = proof.solver_error.is_none() && proof.conflicts.is_empty();

    // Confidence heuristic: exact name match is most predictive.
    let exact_match = rule.name == goal.name;
    let prefix_match = goal.name.starts_with(&rule.name)
        || rule.name.contains(goal.name.as_str());

    let would_solve = (exact_match || prefix_match) && no_conflicts;

    Some(SpeculativeResult {
        solved: would_solve,
        new_goal_ids: vec![], // populated in future workspace-coupled version
        changed: would_solve,
        consistent: no_conflicts,
    })
}

/// Predict applicability without committing.  Returns `true` if `try_rule_on_goal`
/// would return `Some(result)` with `result.changed`.
pub fn is_applicable(proof: &ProofState, goal_id: MorMetaId, rule: &CompiledRule) -> bool {
    try_rule_on_goal(proof, goal_id, rule)
        .map(|r| r.changed || r.solved)
        .unwrap_or(false)
}
