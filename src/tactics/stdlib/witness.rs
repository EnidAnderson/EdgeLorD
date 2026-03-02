//! SC3: Witness insertion tactic.
//!
//! Handles four witness gates: `bc-check` (Beck-Chevalley), `frobenius-check`,
//! `descent-check`, `flat-witness`.  Wraps a goal term `T` with
//! `(gate-symbol ? T)`, producing a hole for the obligation.
//!
//! **INV S-WITNESS**: creates an explicit hole; sound (does not assert truth).

use crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety};
use crate::tactics::query::{TacticQuery, SemanticQuery};
use crate::tactics::edit::EditBuilder;
use crate::document::ByteSpan;
use comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind};
use std::collections::BTreeMap;

const GATES: &[(&str, &str, &str)] = &[
    ("bc-check",        "Beck-Chevalley",     "base-change check"),
    ("frobenius-check", "Frobenius",          "Frobenius reciprocity check"),
    ("descent-check",   "descent",            "descent condition check"),
    ("flat-witness",    "flat",               "flatness witness insertion"),
];

pub struct WitnessInsertTactic;

impl Tactic for WitnessInsertTactic {
    fn id(&self) -> &'static str { "std.insert-witness" }
    fn title(&self) -> &'static str { "Insert witness for proof obligation" }

    fn compute(&self, req: &TacticRequest) -> TacticResult {
        let query = SemanticQuery::new();
        let goal_state = match query.goal_state_at_cursor(req.proof, req.doc, &req.selection) {
            Some(g) => g,
            None => return TacticResult::NotApplicable,
        };
        let goal_span = match goal_state.span {
            Some(s) => ByteSpan::new(s.start, s.end),
            None => return TacticResult::NotApplicable,
        };

        let builder = EditBuilder::new(req.ctx.document_uri().clone(), req.doc.text.clone());
        let goal_text = req.doc.text.get(goal_span.start..goal_span.end).unwrap_or("?goal");

        let mut actions = Vec::new();
        for (gate_sym, law_name, description) in GATES {
            if !gate_relevant(&goal_state.name, gate_sym) {
                continue;
            }
            let new_text = format!("({gate_sym} ? {goal_text})");
            let edit = builder.replace_span(goal_span, new_text.clone());

            let anchor = StableAnchor {
                kind: AnchorKind::Goal,
                file_id: req.ctx.document_uri().to_string(),
                owner_path: vec![],
                ordinal: goal_state.id.as_u32(),
                span_fingerprint: 0,
            };

            let mut metadata = BTreeMap::new();
            metadata.insert("gate".to_string(), (*gate_sym).to_string());
            metadata.insert("law".to_string(), (*law_name).to_string());

            actions.push(TacticAction {
                action_id: format!("{}:{}:{}", self.id(), goal_state.id.as_u32(), gate_sym),
                title: format!("Insert {law_name} witness ({description})"),
                kind: ActionKind::Rewrite,
                safety: ActionSafety::BestEffort,
                anchor,
                edit,
                preview: Some(new_text),
                metadata,
            });
        }

        if actions.is_empty() {
            TacticResult::NotApplicable
        } else {
            TacticResult::Actions(actions)
        }
    }
}

fn gate_relevant(goal_name: &str, gate_sym: &str) -> bool {
    let norm_gate = gate_sym.replace('-', "_");
    let norm_goal = goal_name.to_lowercase();
    norm_goal.contains(gate_sym)
        || norm_goal.contains(&norm_gate)
        || goal_name.contains(gate_sym)
}
