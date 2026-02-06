use crate::tactics::view::Selection;
use crate::document::ParsedDocument;
use crate::document::ByteSpan;
use new_surface_syntax::proof_state::{ProofState, MorMetaId, GoalStatus};
use std::collections::BTreeSet;

/// High-level query interface for tactics.
pub trait TacticQuery {
    /// Find the innermost AST node at the selection.
    fn node_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<ByteSpan>;

    /// Find the goal ID at the selection.
    fn goal_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<MorMetaId>;

    /// Get the set of metas that directly block the given goal.
    fn blockers_for_goal(&self, proof: &ProofState, goal_id: MorMetaId) -> BTreeSet<MorMetaId>;

    /// Find if the selection is inside a macro call and return the macro name.
    fn macro_call_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<String>;
}

pub struct SemanticQuery;

impl SemanticQuery {
    pub fn new() -> Self {
        Self
    }
}

impl TacticQuery for SemanticQuery {
    fn node_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<ByteSpan> {
        let offset = crate::span_conversion::position_to_offset(&doc.text, selection.range.start).unwrap_or(0);
        let chain = doc.selection_chain_for_offset(offset);
        // The chain is sorted from innermost to outermost
        chain.first().copied()
    }

    fn goal_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<MorMetaId> {
        let offset = crate::span_conversion::position_to_offset(&doc.text, selection.range.start).unwrap_or(0);
        let syntactic_goal = doc.goal_at_offset(offset)?;
        
        // We need to resolve the syntactic Goal to a MorMetaId.
        // Usually, goal_id for syntactic goals is "goal-start-end-name".
        // But if we have a stable_id (anchor), we can use the resolver.
        // Actually, TacticRequest should probably resolve this for us or we do it here.
        // For now, let's assume we can find it in ProofState by name if it exists.
        let _ = syntactic_goal; // Avoid unused warning
        None // Placeholder: needs coordination with ProofSession or Index
    }

    fn blockers_for_goal(&self, proof: &ProofState, goal_id: MorMetaId) -> BTreeSet<MorMetaId> {
        let goal = proof.goals.iter().find(|g| g.id == goal_id);
        match goal {
            Some(g) => match &g.status {
                GoalStatus::Blocked { depends_on } => depends_on.iter().cloned().collect(),
                _ => BTreeSet::new(),
            },
            None => BTreeSet::new(),
        }
    }

    fn macro_call_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<String> {
        let offset = crate::span_conversion::position_to_offset(&doc.text, selection.range.start).unwrap_or(0);
        let _chain = doc.selection_chain_for_offset(offset);
        // TODO: Inspect SExpr at this span to see if it's a list starting with a macro name
        None
    }
}

impl Default for SemanticQuery {
    fn default() -> Self {
        Self::new()
    }
}
