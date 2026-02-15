/// Loogle code actions for lemma suggestions
use crate::loogle::{check_applicability, to_proposal, LoogleResult};
use tower_lsp::lsp_types::{CodeAction, CodeActionKind, WorkspaceEdit, TextEdit, Range};
use std::collections::HashMap;
use comrade_lisp::proof_state::ProofState;

/// Generate Loogle-based code actions for the current cursor position
pub fn generate_loogle_actions(
    indexer: &crate::loogle::WorkspaceIndexer,
    proof_state: &ProofState,
    range: Range,
    uri: &tower_lsp::lsp_types::Url,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    
    // Get current goal at cursor position if available
    // For MVP, just search all lemmas and show top 3
    let search_results = indexer.index().search("", 5).unwrap_or_default();
    
    for (idx, lemma) in search_results.iter().take(3).enumerate() {
        // Check applicability (for now, always show but with varying confidence)
        let applicability = check_applicability(lemma, "");
        
        if applicability.confidence > 0.1 {
            let action = create_apply_lemma_action(lemma, range, uri, idx);
            actions.push(action);
        }
    }
    
    actions
}

fn create_apply_lemma_action(
    lemma: &LoogleResult,
    range: Range,
    uri: &tower_lsp::lsp_types::Url,
    priority: usize,
) -> CodeAction {
    let title = format!("🔍 Apply lemma: {}", lemma.name);
    
    // Create a workspace edit that inserts the lemma application
    let text_edit = TextEdit {
        range,
        new_text: format!("apply {}\n", lemma.name),
    };
    
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), vec![text_edit]);
    
    let edit = WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    };
    
    CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: Some(priority == 0),
        disabled: None,
        data: None,
    }
}
