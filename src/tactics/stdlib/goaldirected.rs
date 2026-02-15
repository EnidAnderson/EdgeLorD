use crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety};
use crate::tactics::edit::EditBuilder;
use comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind};
use std::collections::BTreeMap;

pub struct FocusGoalTactic;

impl Tactic for FocusGoalTactic {
    fn id(&self) -> &'static str {
        "std.focus_goal"
    }

    fn title(&self) -> &'static str {
        "Focus on this goal"
    }

    fn compute(&self, req: &TacticRequest) -> TacticResult {
        let offset = crate::span_conversion::position_to_offset(&req.doc.text, req.selection.range.start).unwrap_or(0);
        let Some(goal) = req.doc.goal_at_offset(offset) else {
            return TacticResult::NotApplicable;
        };

        let builder = EditBuilder::new(req.ctx.document_uri().clone(), req.doc.text.clone());
        // Wrap goal span in (focus ... span)
        // We'll use a simple edit that replaces the span with (focus (goal))
        // or actually just prepends (focus ) and appends ).
        let edit = builder.wrap_span(goal.span, "(focus ".to_string(), ")".to_string());

        let anchor = StableAnchor {
            kind: AnchorKind::Goal,
            file_id: req.ctx.document_uri().to_string(),
            owner_path: vec![],
            ordinal: 0,
            span_fingerprint: 0,
        };

        let action = TacticAction {
            action_id: format!("{}:{}", self.id(), goal.goal_id),
            title: "Focus on this goal".to_string(),
            kind: ActionKind::Refactor,
            safety: ActionSafety::Safe,
            anchor,
            edit,
            preview: Some("(focus (...) )".to_string()),
            metadata: BTreeMap::new(),
        };

        TacticResult::Actions(vec![action])
    }
}
