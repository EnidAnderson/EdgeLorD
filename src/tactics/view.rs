use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tower_lsp::lsp_types::{Range, WorkspaceEdit};
use new_surface_syntax::proof_state::ProofState;
use crate::document::ParsedDocument;
use crate::edgelord_pretty_ctx::EdgeLordPrettyCtx;
use new_surface_syntax::diagnostics::anchors::StableAnchor;
use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;

/// Selection details from the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    /// The range selected. If start == end, it's a cursor position.
    pub range: Range,
}

/// Resource limits for tactic computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticLimits {
    /// Maximum time allowed for computation (default 100ms).
    pub timeout_ms: u64,
}

impl Default for TacticLimits {
    fn default() -> Self {
        Self { timeout_ms: 100 }
    }
}

/// Input bundle for a tactic.
pub struct TacticRequest<'a> {
    pub ctx: &'a EdgeLordPrettyCtx<'a>,
    pub proof: &'a ProofState,
    pub doc: &'a ParsedDocument,
    pub index: Option<&'a GoalsPanelIndex>,
    pub selection: Selection,
    pub limits: TacticLimits,
}

/// The result of a tactic proposal.
pub enum TacticResult {
    /// Tactic does not apply to this selection.
    NotApplicable,
    /// Successfully generated one or more proposals.
    Actions(Vec<TacticAction>),
    /// Proposals were generated but truncated due to limits.
    Truncated {
        actions: Vec<TacticAction>,
        reason: String,
    },
}

/// Safety level of a tactic action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActionSafety {
    /// Semantics-preserving, should always re-elaborate successfully.
    Safe,
    /// Heuristic-based, may fail elaboration but usually reasonable.
    BestEffort,
    /// Changes meaning or removes data; always requires user confirmation.
    Destructive,
}

/// Kind of tactic action for UI grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    QuickFix,
    Refactor,
    Rewrite,
    Explain,
    Expand,
}

/// A specific proposal from a tactic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticAction {
    /// Stable ID for this specific action (e.g., "std.add_touch:anchor123").
    pub action_id: String,
    /// Human-readable title shown in the CodeAction menu.
    pub title: String,
    /// UI grouping.
    pub kind: ActionKind,
    /// Safety label.
    pub safety: ActionSafety,
    /// The AST anchor this action targets.
    pub anchor: StableAnchor,
    /// The actual edit to apply.
    pub edit: WorkspaceEdit,
    /// Optional preview of the change.
    pub preview: Option<String>,
    /// Deterministically sorted metadata.
    pub metadata: BTreeMap<String, String>,
}

/// Interface for all tactics.
pub trait Tactic: Send + Sync {
    /// Stable identifier for registry (e.g., "std.add_touch").
    fn id(&self) -> &'static str;
    /// Human-readable title of the tactic family.
    fn title(&self) -> &'static str;
    /// Compute proposals for a given request.
    fn compute(&self, req: &TacticRequest) -> TacticResult;
}
