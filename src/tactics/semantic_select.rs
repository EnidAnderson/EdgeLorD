//! SE2: Avy-jump semantic selection protocol.
//!
//! A `SemanticSelection` records the user's chosen pattern occurrence.
//! Tactics (especially SE3 multi-rewrite) use it to target a specific subterm.

use serde::{Deserialize, Serialize};

use crate::tactics::pattern_find::{OccurrenceScope, PatternOccurrence};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSelection {
    pub pattern_name: String,
    pub occurrence: PatternOccurrence,
    pub source_rule: Option<String>,
    pub scope: OccurrenceScope,
}

/// Given a list of occurrences and a label typed by the user, find the matching occurrence.
///
/// **INV D-***: label assignment must match `generate_label(index)`.
pub fn find_occurrence_by_label<'a>(
    occurrences: &'a [PatternOccurrence],
    label: &str,
) -> Option<&'a PatternOccurrence> {
    occurrences
        .iter()
        .enumerate()
        .find(|(i, _)| crate::tactics::pattern_find::generate_label(*i) == label)
        .map(|(_, occ)| occ)
}
