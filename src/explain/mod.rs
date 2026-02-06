pub mod view;
pub mod builder;
pub mod alg_goal;
pub mod alg_blocked;
pub mod alg_inconsistent;

use tower_lsp::jsonrpc::Result;
use crate::explain::view::{ExplainRequest, ExplanationView, ExplainTarget, ExplainLimits, ExplanationNode, validate_span};
use crate::proof_session::ProofSession;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main handler for edgelord/explain requests.
pub async fn handle_explain_request(
    req: ExplainRequest,
    proof_session: Arc<RwLock<ProofSession>>,
) -> Result<ExplanationView> {
    let session = proof_session.read().await;
    
    // 1. Resolve Document
    let doc = session.get_document(&req.uri)
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Document not found"))?;
    
    let text_len = doc.parsed.text.len();
    
    // 2. Resolve ProofState
    let proof_state = doc.workspace_report.proof_state.as_ref()
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("No proof state available"))?;
    
    let index = doc.goals_index.as_ref()
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("No goals index available"))?;
    
    // 3. Setup Limits (with hard caps)
    let limits = enforce_hard_caps(req.limits.unwrap_or_default());
    
    // 4. Call Pure Algorithm
    let mut view = match req.target {
        ExplainTarget::Goal { ref goal_id } => {
            alg_goal::explain_goal(goal_id, proof_state, index, limits)
        }
        ExplainTarget::WhyBlocked { ref goal_id } => {
            alg_blocked::explain_why_blocked(goal_id, proof_state, index, limits)
        }
        ExplainTarget::WhyInconsistent { ref goal_id } => {
            alg_inconsistent::explain_why_inconsistent(goal_id, proof_state, index, limits)
        }
        _ => return Err(tower_lsp::jsonrpc::Error::method_not_found()),
    };
    
    // 5. Last Line of Defense: Validate Jump Targets
    validate_view_spans(&mut view.root, text_len);
    
    Ok(view)
}

fn enforce_hard_caps(mut limits: ExplainLimits) -> ExplainLimits {
    const MAX_NODES_CAP: usize = 300;
    const MAX_DEPTH_CAP: usize = 50;
    const MAX_CHILDREN_CAP: usize = 50;
    const MAX_LABEL_CHARS_CAP: usize = 5000;
    const MAX_TIMEOUT_MS_CAP: u64 = 1000;
    
    limits.max_nodes = limits.max_nodes.min(MAX_NODES_CAP);
    limits.max_depth = limits.max_depth.min(MAX_DEPTH_CAP);
    limits.max_children_per_node = limits.max_children_per_node.min(MAX_CHILDREN_CAP);
    limits.max_label_chars = limits.max_label_chars.min(MAX_LABEL_CHARS_CAP);
    limits.timeout_ms = limits.timeout_ms.min(MAX_TIMEOUT_MS_CAP);
    
    limits
}

fn validate_view_spans(node: &mut ExplanationNode, text_len: usize) {
    if let Some(span) = node.jump_target {
        node.jump_target = validate_span(span, text_len);
    }
    for child in &mut node.children {
        validate_view_spans(child, text_len);
    }
}
