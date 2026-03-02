//! SE1: Pattern highlight overlay — visualize all pattern match sites.
//!
//! Produces a `PatternOverlayParams` from SE0 occurrences.
//!
//! **INV D-***: labels assigned by sorted occurrence order.

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{Position, Range};

use crate::document::ByteSpan;
use crate::tactics::pattern_find::{generate_label, OccurrenceScope, PatternOccurrence};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternOverlayItem {
    pub range: Range,
    pub label: String,
    pub tooltip: String,
    pub confidence: String, // "exact" | "head-match" | "name-contains"
    pub goal_id: String,
    pub occurrence_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternOverlayParams {
    pub uri: String,
    pub pattern_label: String,
    pub occurrences: Vec<PatternOverlayItem>,
    pub scope: String,
}

/// Convert SE0 occurrences to overlay items with labels.
pub fn build_overlay(
    occurrences: &[PatternOccurrence],
    uri: &str,
    pattern_label: &str,
    doc_text: &str,
    scope: &OccurrenceScope,
) -> PatternOverlayParams {
    let scope_str = match scope {
        OccurrenceScope::FocusedGoal(_) => "focused",
        OccurrenceScope::UnsolvedGoals => "unsolved",
        OccurrenceScope::AllGoals => "all",
        OccurrenceScope::GoalsAndContext => "with-context",
        OccurrenceScope::Everything => "everything",
    };

    let items: Vec<PatternOverlayItem> = occurrences
        .iter()
        .enumerate()
        .map(|(i, occ)| {
            let range = occ
                .span
                .map(|s| span_to_range(s, doc_text))
                .unwrap_or_default();
            let confidence = if occ.matched_text == pattern_label {
                "exact"
            } else if occ.matched_text.contains(pattern_label) {
                "head-match"
            } else {
                "name-contains"
            };
            let binding_text = occ
                .bindings
                .iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            let tooltip = if binding_text.is_empty() {
                format!("Match at {}", format_site(&occ.site))
            } else {
                format!("Match at {}: {}", format_site(&occ.site), binding_text)
            };

            PatternOverlayItem {
                range,
                label: generate_label(i),
                tooltip,
                confidence: confidence.to_string(),
                goal_id: occ.goal_id.to_string(),
                occurrence_index: i,
            }
        })
        .collect();

    PatternOverlayParams {
        uri: uri.to_string(),
        pattern_label: pattern_label.to_string(),
        occurrences: items,
        scope: scope_str.to_string(),
    }
}

fn format_site(site: &crate::tactics::pattern_find::MatchSite) -> String {
    use crate::tactics::pattern_find::MatchSite;
    match site {
        MatchSite::GoalTarget { side } => format!("goal target ({:?})", side),
        MatchSite::Hypothesis { name } => format!("hypothesis {}", name),
    }
}

fn span_to_range(span: ByteSpan, text: &str) -> Range {
    let start = crate::span_conversion::offset_to_position(text, span.start)
        .unwrap_or_else(|| Position::new(0, 0));
    let end = crate::span_conversion::offset_to_position(text, span.end)
        .unwrap_or_else(|| Position::new(0, 0));
    Range { start, end }
}

/// Public re-export of span_to_range for use in multi_rewrite.
pub fn span_to_range_pub(span: ByteSpan, text: &str) -> Range {
    span_to_range(span, text)
}
