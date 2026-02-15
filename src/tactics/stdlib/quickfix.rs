use crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety};
use crate::tactics::edit::EditBuilder;
use comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind};
use std::collections::BTreeMap;

pub struct AddTouchTactic;

impl Tactic for AddTouchTactic {
    fn id(&self) -> &'static str {
        "std.add_touch"
    }

    fn title(&self) -> &'static str {
        "Add missing touch"
    }

    fn compute(&self, req: &TacticRequest) -> TacticResult {
        let offset = crate::span_conversion::position_to_offset(&req.doc.text, req.selection.range.start).unwrap_or(0);
        let Some(goal) = req.doc.goal_at_offset(offset) else {
            return TacticResult::NotApplicable;
        };

        let Some(name) = &goal.name else {
            return TacticResult::NotApplicable;
        };

        // Simple heuristic: if the name is already in the document as (touch name), skip.
        if req.doc.text.contains(&format!("(touch {})", name)) {
            return TacticResult::NotApplicable;
        }

        let builder = EditBuilder::new(req.ctx.document_uri().clone(), req.doc.text.clone());
        let edit = builder.insert_before_span(goal.span, format!("(touch {})\n", name));

        // Attempt to resolve anchor
        let anchor = if let Some(stable_id) = &goal.stable_id {
            // If we have a stable ID string, we ideally want to parse it back to a StableAnchor.
            // For now, we'll construct a representative one.
            let _ = stable_id; // Avoid unused warning
            StableAnchor {
                kind: AnchorKind::Goal,
                file_id: req.ctx.document_uri().to_string(),
                owner_path: vec![],
                ordinal: 0,
                span_fingerprint: 0,
            }
        } else {
            // Fallback to a syntactic anchor
            StableAnchor {
                kind: AnchorKind::AstNode,
                file_id: req.ctx.document_uri().to_string(),
                owner_path: vec!["syntactic".to_string()],
                ordinal: offset as u32,
                span_fingerprint: 0,
            }
        };

        let action = TacticAction {
            action_id: format!("{}:{}", self.id(), goal.goal_id),
            title: format!("Add (touch {name}) before this goal"),
            kind: ActionKind::QuickFix,
            safety: ActionSafety::Safe,
            anchor,
            edit,
            preview: Some(format!("+(touch {name})")),
            metadata: BTreeMap::new(),
        };

        TacticResult::Actions(vec![action])
    }
}
