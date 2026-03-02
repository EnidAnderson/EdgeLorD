//! SC4: Kernel tactic adapter.
//!
//! `KernelTacticAdapter<T>` implements the EdgeLorD `Tactic` trait by wrapping
//! a `KernelTactic` from `comrade_lisp::tactic_protocol`.  It bridges:
//!
//!   `TacticRequest`  →  `TacticInput`  →  `KernelTactic::execute`
//!                    →  `TacticOutput`  →  `TacticAction` (WorkspaceEdit)
//!
//! **INV S-SINGLE-MODEL**: kernel tactics never certify; the adapter only
//! converts their `ProofStateDelta` into text edits for user approval.
//! **INV T-Boundary**: `KernelTacticAdapter` calls no Stonewall guards.
//! **INV D-***: deterministic — same inputs produce the same actions.

use comrade_lisp::tactic_protocol::{
    KernelTactic, TacticInput, FocusId, ProofStateDelta, SolutionPayload,
    StableAnchor as KernelAnchor, TacticError, TacticOutput,
};
use crate::tactics::view::{Tactic, TacticRequest, TacticResult, TacticAction, ActionKind, ActionSafety};
use crate::tactics::query::{TacticQuery, SemanticQuery};
use crate::tactics::edit::EditBuilder;
use comrade_lisp::diagnostics::anchors::{StableAnchor, AnchorKind};
use comrade_lisp::proof_state::MorMetaId;
use crate::document::ByteSpan;
use std::collections::BTreeMap;

// ─── Adapter ──────────────────────────────────────────────────────────────────

/// Wraps a `T: KernelTactic` as an EdgeLorD `Tactic`.
pub struct KernelTacticAdapter<T: KernelTactic + Send + Sync + 'static> {
    id: &'static str,
    title: &'static str,
    inner: T,
    action_kind: ActionKind,
    safety: ActionSafety,
}

impl<T: KernelTactic + Send + Sync + 'static> KernelTacticAdapter<T> {
    pub fn new(
        id: &'static str,
        title: &'static str,
        inner: T,
        action_kind: ActionKind,
        safety: ActionSafety,
    ) -> Self {
        Self { id, title, inner, action_kind, safety }
    }
}

impl<T: KernelTactic + Send + Sync + 'static> Tactic for KernelTacticAdapter<T> {
    fn id(&self) -> &'static str { self.id }
    fn title(&self) -> &'static str { self.title }

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

        let kernel_anchor = KernelAnchor::new(goal_state.id, goal_state.span);

        let input = TacticInput {
            focus: FocusId::Goal(goal_state.id),
            proof_state: req.proof.clone(),
            local_context: goal_state.local_context.clone(),
            surface_anchor: kernel_anchor,
        };

        let output: TacticOutput = match self.inner.execute(&input) {
            Ok(o) => o,
            Err(TacticError::InvalidInput(_)) => return TacticResult::NotApplicable,
            Err(_) => return TacticResult::NotApplicable,
        };

        let builder = EditBuilder::new(req.ctx.document_uri().clone(), req.doc.text.clone());
        let actions = delta_to_actions(
            &output.proof_state_delta,
            goal_state.id,
            goal_span,
            req.ctx.document_uri().to_string(),
            self.id,
            self.action_kind,
            self.safety,
            &builder,
        );

        if actions.is_empty() {
            TacticResult::NotApplicable
        } else {
            TacticResult::Actions(actions)
        }
    }
}

// ─── ExactTactic ─────────────────────────────────────────────────────────────

/// Kernel tactic: close a goal by exact hypothesis match.
///
/// If any hypothesis in the local context has a type that equals the goal's
/// expected type exactly, produces a `SolveGoal` delta with that hypothesis.
///
/// **INV T-Boundary**: no Stonewall calls; pure function.
/// **INV D-***: iterates `local_context.entries` deterministically.
pub struct ExactTactic;

impl KernelTactic for ExactTactic {
    fn execute(&self, input: &TacticInput) -> Result<TacticOutput, TacticError> {
        let goal_id = match input.focus {
            FocusId::Goal(id) => id,
            _ => return Err(TacticError::InvalidInput("ExactTactic requires Goal focus".into())),
        };

        let goal = input.proof_state.goals.iter()
            .find(|g| g.id == goal_id)
            .ok_or_else(|| TacticError::InvalidInput("Goal not found".into()))?;

        // Scan local context for an exact type match (deterministic — Vec<CtxEntry>)
        let match_name = input.local_context.entries.iter()
            .find(|entry| entry.ty.as_ref() == Some(&goal.expected_type))
            .map(|entry| entry.name.clone());

        match match_name {
            Some(hyp_name) => Ok(TacticOutput {
                proof_state_delta: ProofStateDelta::SolveGoal {
                    goal_id,
                    solution: SolutionPayload::Text(hyp_name),
                },
                text_edits: vec![],
                diagnostics: vec![],
            }),
            None => Ok(TacticOutput {
                proof_state_delta: ProofStateDelta::NoChange,
                text_edits: vec![],
                diagnostics: vec![],
            }),
        }
    }
}

// ─── Delta → actions conversion ──────────────────────────────────────────────

fn delta_to_actions(
    delta: &ProofStateDelta,
    goal_id: MorMetaId,
    goal_span: ByteSpan,
    file_id: String,
    tactic_id: &str,
    action_kind: ActionKind,
    safety: ActionSafety,
    builder: &EditBuilder,
) -> Vec<TacticAction> {
    match delta {
        ProofStateDelta::NoChange => vec![],

        ProofStateDelta::SolveGoal { goal_id: gid, solution } => {
            if *gid != goal_id { return vec![]; }
            let text = solution_to_text(solution);
            let edit = builder.replace_span(goal_span, text.clone());
            vec![TacticAction {
                action_id: format!("{tactic_id}:{}", goal_id.as_u32()),
                title: format!("Solve by {text}"),
                kind: action_kind,
                safety,
                anchor: goal_anchor(goal_id, file_id),
                edit,
                preview: Some(text),
                metadata: BTreeMap::new(),
            }]
        }

        ProofStateDelta::Multiple(deltas) => {
            let mut all = Vec::new();
            for d in deltas {
                all.extend(delta_to_actions(
                    d, goal_id, goal_span, file_id.clone(),
                    tactic_id, action_kind, safety, builder,
                ));
            }
            all
        }

        _ => vec![],
    }
}

fn solution_to_text(payload: &SolutionPayload) -> String {
    match payload {
        SolutionPayload::Text(s) => s.clone(),
        SolutionPayload::Surface(sexpr) => format!("{:?}", sexpr),
        SolutionPayload::Core(_) => "(kernel-solution)".to_string(),
    }
}

fn goal_anchor(goal_id: MorMetaId, file_id: String) -> StableAnchor {
    StableAnchor {
        kind: AnchorKind::Goal,
        file_id,
        owner_path: vec![],
        ordinal: goal_id.as_u32(),
        span_fingerprint: 0,
    }
}
