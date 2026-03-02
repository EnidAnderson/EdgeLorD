//! SC2: Rewrite tactic — doctrine-aware rule application proposals.
//!
//! Uses SC0 (`RuleIndex`) for rule discovery and SC1 (`try_rule_on_goal`) for
//! speculative applicability previews.
//!
//! **INV D-***: actions sorted deterministically; identical request = identical output.

use crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety};
use crate::tactics::rule_index::MatchConfidence;
use crate::tactics::speculative::try_rule_on_goal;
use crate::tactics::query::{TacticQuery, SemanticQuery};
use crate::tactics::edit::EditBuilder;
use crate::document::ByteSpan;
use comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind};
use std::collections::BTreeMap;

const MAX_REWRITE_ACTIONS: usize = 10;

pub struct RewriteTactic;

impl Tactic for RewriteTactic {
    fn id(&self) -> &'static str { "std.rewrite" }
    fn title(&self) -> &'static str { "Apply rewrite rule" }

    fn compute(&self, req: &TacticRequest) -> TacticResult {
        let rule_index = match req.rule_index {
            Some(ri) => ri,
            None => return TacticResult::NotApplicable,
        };

        let query = SemanticQuery::new();
        let goal_state = match query.goal_state_at_cursor(req.proof, req.doc, &req.selection) {
            Some(g) => g,
            None => return TacticResult::NotApplicable,
        };

        let goal_span = match goal_state.span {
            Some(s) => ByteSpan::new(s.start, s.end),
            None => return TacticResult::NotApplicable,
        };

        let candidates = rule_index.rules_matching_goal(goal_state);
        if candidates.is_empty() {
            return TacticResult::NotApplicable;
        }

        let total = candidates.len();
        let builder = EditBuilder::new(req.ctx.document_uri().clone(), req.doc.text.clone());
        let mut actions = Vec::new();

        for rm in candidates.iter().take(MAX_REWRITE_ACTIONS) {
            let spec = try_rule_on_goal(req.proof, goal_state.id, rm.rule);

            let confidence_label = match rm.confidence {
                MatchConfidence::Exact        => "exact",
                MatchConfidence::Unifiable    => "unifiable",
                MatchConfidence::HeadMatch    => "head match",
                MatchConfidence::DoctrineOnly => "doctrine",
            };
            let witness_suffix = if rm.needs_witness { " \u{26a0} needs witness" } else { "" };
            let solve_mark = if spec.as_ref().map(|s| s.solved).unwrap_or(false) { " \u{2713}" } else { "" };

            let title = format!(
                "Apply `{}`  \u{2014} {confidence_label}{witness_suffix}{solve_mark}",
                rm.rule.name
            );

            let new_text = format!("(apply {})", rm.rule.name);
            let edit = builder.replace_span(goal_span, new_text.clone());

            let anchor = StableAnchor {
                kind: AnchorKind::Goal,
                file_id: req.ctx.document_uri().to_string(),
                owner_path: vec![],
                ordinal: goal_state.id.as_u32(),
                span_fingerprint: 0,
            };

            let mut metadata = BTreeMap::new();
            metadata.insert("rule".to_string(), rm.rule.name.clone());
            metadata.insert("confidence".to_string(), confidence_label.to_string());
            metadata.insert("needs_witness".to_string(), rm.needs_witness.to_string());
            if let Some(d) = &rm.rule.doctrine_context {
                metadata.insert("doctrine".to_string(), d.clone());
            }

            actions.push(TacticAction {
                action_id: format!("{}:{}:{}", self.id(), goal_state.id.as_u32(), rm.rule.name),
                title,
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
        } else if total > MAX_REWRITE_ACTIONS {
            TacticResult::Truncated {
                actions,
                reason: format!(
                    "{} more rules available; showing top {MAX_REWRITE_ACTIONS}",
                    total - MAX_REWRITE_ACTIONS
                ),
            }
        } else {
            TacticResult::Actions(actions)
        }
    }
}
