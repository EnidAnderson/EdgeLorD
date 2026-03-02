use crate::tactics::view::Selection;
use crate::document::ParsedDocument;
use crate::document::ByteSpan;
use comrade_lisp::proof_state::{ProofState, MorMetaId, GoalStatus, GoalState};
use std::collections::BTreeSet;

/// High-level query interface for tactics.
pub trait TacticQuery {
    /// Find the innermost AST node at the selection.
    fn node_at_cursor(&self, doc: &ParsedDocument, selection: &Selection) -> Option<ByteSpan>;

    /// Find the `MorMetaId` of the goal whose source span encloses the cursor.
    ///
    /// Matches the syntactic `Goal` at the cursor offset (by name) to a `GoalState`
    /// in the authoritative `ProofState`. Returns `None` if the cursor is not inside
    /// any goal span or no `GoalState` with that name exists.
    ///
    /// **INV D-*:** deterministic — same inputs produce same output.
    fn goal_at_cursor(
        &self,
        proof: &ProofState,
        doc: &ParsedDocument,
        selection: &Selection,
    ) -> Option<MorMetaId>;

    /// Like `goal_at_cursor` but returns a reference to the full `GoalState`.
    fn goal_state_at_cursor<'p>(
        &self,
        proof: &'p ProofState,
        doc: &ParsedDocument,
        selection: &Selection,
    ) -> Option<&'p GoalState>;

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

    fn goal_at_cursor(
        &self,
        proof: &ProofState,
        doc: &ParsedDocument,
        selection: &Selection,
    ) -> Option<MorMetaId> {
        self.goal_state_at_cursor(proof, doc, selection).map(|g| g.id)
    }

    fn goal_state_at_cursor<'p>(
        &self,
        proof: &'p ProofState,
        doc: &ParsedDocument,
        selection: &Selection,
    ) -> Option<&'p GoalState> {
        let offset = crate::span_conversion::position_to_offset(&doc.text, selection.range.start).unwrap_or(0);
        let syntactic_goal = doc.goal_at_offset(offset)?;

        // Resolve name: goal_id is "?name"; goal.name is the bare name.
        let goal_name: &str = syntactic_goal
            .name
            .as_deref()
            .or_else(|| syntactic_goal.goal_id.strip_prefix('?'))?;

        // Find smallest-span GoalState in ProofState whose name matches.
        // If multiple GoalStates share a name (shouldn't happen but be safe),
        // prefer the one whose span start is closest to the cursor offset.
        let best = proof.goals.iter().filter(|g| g.name == goal_name).min_by_key(|g| {
            let span_start = g.span.map(|s| s.start).unwrap_or(0);
            (span_start as i64 - offset as i64).unsigned_abs() as u64
        });
        best
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
