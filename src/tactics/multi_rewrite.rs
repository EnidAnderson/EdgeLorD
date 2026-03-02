//! SE3: Multi-site rewrite — apply a rule at multiple pattern match sites.
//!
//! **INV D-***: sites processed in deterministic order (by occurrence index).
//! **INV S-SPECULATIVE**: composite result is advisory; never certified.

use std::collections::HashMap;

use comrade_lisp::core::CompiledRule;
use comrade_lisp::proof_state::{MorMetaId, ProofState};
use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{TextEdit, WorkspaceEdit};

use crate::document::ByteSpan;
use crate::tactics::pattern_find::PatternOccurrence;
use crate::tactics::speculative::try_rule_on_goal;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiSiteStrategy {
    All,
    Selected(Vec<usize>),
    Exhaust { max_rounds: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub site_a: usize,
    pub site_b: usize,
    pub kind: String, // "overlapping-spans" | "causal-dependency"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRewriteResult {
    pub applied_sites: Vec<usize>,
    pub skipped_sites: Vec<(usize, String)>,
    pub conflicts: Vec<ConflictInfo>,
    pub total_sites: usize,
    pub summary: String,
}

// ─── Engine ──────────────────────────────────────────────────────────────────

/// Build a composite `WorkspaceEdit` from non-overlapping occurrences.
pub fn build_composite_edit(
    occurrences: &[PatternOccurrence],
    doc_uri: &tower_lsp::lsp_types::Url,
    doc_text: &str,
    rule: &CompiledRule,
) -> (WorkspaceEdit, Vec<ConflictInfo>) {
    let mut conflicts = Vec::new();
    let mut used_spans: Vec<(usize, ByteSpan)> = Vec::new();
    let mut edits: Vec<TextEdit> = Vec::new();

    for (i, occ) in occurrences.iter().enumerate() {
        let span = match occ.span {
            Some(s) => s,
            None => continue,
        };

        // Check for overlap with already-applied spans
        let mut overlaps = false;
        for (j, (_, used)) in used_spans.iter().enumerate() {
            if span.start < used.end && used.start < span.end {
                conflicts.push(ConflictInfo {
                    site_a: j,
                    site_b: i,
                    kind: "overlapping-spans".to_string(),
                });
                overlaps = true;
                break;
            }
        }
        if overlaps {
            continue;
        }

        let range = crate::pattern_overlay::span_to_range_pub(span, doc_text);
        let new_text = format!("(apply {})", rule.name);
        edits.push(TextEdit { range, new_text });
        used_spans.push((i, span));
    }

    let mut changes = HashMap::new();
    if !edits.is_empty() {
        changes.insert(doc_uri.clone(), edits);
    }
    let edit = WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    };
    (edit, conflicts)
}

/// Process multi-site rewrite request.
pub fn multi_site_rewrite(
    proof_state: &ProofState,
    rule: &CompiledRule,
    occurrences: Vec<PatternOccurrence>,
    strategy: MultiSiteStrategy,
    doc_uri: &tower_lsp::lsp_types::Url,
    doc_text: &str,
) -> MultiRewriteResult {
    let total = occurrences.len();

    let target_indices: Vec<usize> = match &strategy {
        MultiSiteStrategy::All | MultiSiteStrategy::Exhaust { .. } => (0..total).collect(),
        MultiSiteStrategy::Selected(idxs) => idxs.clone(),
    };

    let targets: Vec<&PatternOccurrence> = target_indices
        .iter()
        .filter_map(|&i| occurrences.get(i))
        .collect();

    // Speculative check per site
    let mut applicable: Vec<PatternOccurrence> = Vec::new();
    let mut skipped = Vec::new();
    for (local_i, occ) in targets.iter().enumerate() {
        let goal_id = MorMetaId(occ.goal_id);
        let spec = try_rule_on_goal(proof_state, goal_id, rule);
        if spec.map(|s| s.changed || s.solved).unwrap_or(false) {
            applicable.push((*occ).clone());
        } else {
            skipped.push((local_i, "not applicable speculatively".to_string()));
        }
    }

    let (_, conflicts) = build_composite_edit(&applicable, doc_uri, doc_text, rule);
    let applied_count = applicable.len().saturating_sub(conflicts.len());

    MultiRewriteResult {
        applied_sites: (0..applied_count).collect(),
        skipped_sites: skipped,
        conflicts,
        total_sites: total,
        summary: format!("Applied at {applied_count}/{total} sites"),
    }
}
