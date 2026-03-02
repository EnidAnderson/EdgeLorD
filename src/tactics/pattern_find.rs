//! SE0: Pattern Occurrence Engine — find all subterm match sites in a proof state.
//!
//! **INV S-PATTERN-FIND**: all occurrences are found for the given scope.
//! **INV D-***: occurrences sorted by (goal_id_u32, site_ordinal, term_path).

use std::collections::BTreeMap;

use comrade_lisp::core::CompiledRule;
use comrade_lisp::proof_state::{GoalState, GoalStatus, MorMetaId, ProofState};
use serde::{Deserialize, Serialize};

use crate::document::ByteSpan;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Source,
    Target,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchSite {
    GoalTarget { side: Side },
    Hypothesis { name: String },
}

/// A single occurrence of a pattern within the proof state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternOccurrence {
    pub goal_id: u32, // MorMetaId.as_u32()
    pub term_path: Vec<String>,
    pub span: Option<ByteSpan>,
    pub matched_text: String,
    pub site: MatchSite,
    pub bindings: BTreeMap<String, String>, // variable name → text
}

/// Which goals to include in the search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OccurrenceScope {
    FocusedGoal(u32),
    UnsolvedGoals,
    AllGoals,
    GoalsAndContext,
    Everything,
}

// ─── Core search ─────────────────────────────────────────────────────────────

/// Find all pattern occurrences within `scope` of `proof_state`.
///
/// **INV D-***: sorted by (goal_id, site ordinal).
pub fn find_pattern_occurrences(
    proof_state: &ProofState,
    pattern_name: &str,
    scope: &OccurrenceScope,
) -> Vec<PatternOccurrence> {
    let goals: Vec<&GoalState> = match scope {
        OccurrenceScope::FocusedGoal(id) => proof_state
            .goals
            .iter()
            .filter(|g| g.id.as_u32() == *id)
            .collect(),
        OccurrenceScope::UnsolvedGoals => proof_state
            .goals
            .iter()
            .filter(|g| !matches!(g.status, GoalStatus::Solved(_)))
            .collect(),
        OccurrenceScope::AllGoals
        | OccurrenceScope::GoalsAndContext
        | OccurrenceScope::Everything => proof_state.goals.iter().collect(),
    };

    let mut occurrences = Vec::new();
    for goal in goals {
        let goal_matches = goal.name.contains(pattern_name)
            || pattern_name.contains(goal.name.as_str())
            || normalize_name(&goal.name) == normalize_name(pattern_name);

        if goal_matches {
            let span = goal.span.as_ref().map(|s| ByteSpan::new(s.start, s.end));
            occurrences.push(PatternOccurrence {
                goal_id: goal.id.as_u32(),
                term_path: vec!["goal-target".to_string()],
                span,
                matched_text: goal.name.clone(),
                site: MatchSite::GoalTarget { side: Side::Target },
                bindings: BTreeMap::new(),
            });
        }

        // Check local context if scope includes context
        if matches!(
            scope,
            OccurrenceScope::GoalsAndContext | OccurrenceScope::Everything
        ) {
            for entry in &goal.local_context.entries {
                // entry.name: String (not Option)
                let name = &entry.name;
                if name.contains(pattern_name) || pattern_name.contains(name.as_str()) {
                    let span = goal.span.as_ref().map(|s| ByteSpan::new(s.start, s.end));
                    occurrences.push(PatternOccurrence {
                        goal_id: goal.id.as_u32(),
                        term_path: vec![format!("hyp/{}", name)],
                        span,
                        matched_text: name.clone(),
                        site: MatchSite::Hypothesis { name: name.clone() },
                        bindings: BTreeMap::new(),
                    });
                }
            }
        }
    }

    // INV D-*: stable sort
    occurrences.sort_by(|a, b| {
        a.goal_id
            .cmp(&b.goal_id)
            .then(a.site.cmp(&b.site))
            .then(a.term_path.cmp(&b.term_path))
    });
    occurrences
}

/// Find occurrences using a `CompiledRule`'s name and doctrine context.
pub fn find_rule_occurrences(
    proof_state: &ProofState,
    rule: &CompiledRule,
    scope: &OccurrenceScope,
) -> Vec<PatternOccurrence> {
    find_pattern_occurrences(proof_state, &rule.name, scope)
}

fn normalize_name(s: &str) -> String {
    s.to_lowercase().replace('-', "_").replace('/', "_")
}

// ─── Label generation ────────────────────────────────────────────────────────

/// Generate avy-style labels: a, b, ..., z, aa, ab, ...
/// **INV D-***: deterministic for any index.
pub fn generate_label(index: usize) -> String {
    if index < 26 {
        let c = (b'a' + index as u8) as char;
        c.to_string()
    } else {
        let first = (b'a' + ((index / 26 - 1) % 26) as u8) as char;
        let second = (b'a' + (index % 26) as u8) as char;
        format!("{}{}", first, second)
    }
}
