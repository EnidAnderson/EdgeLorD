use serde::{Serialize, Deserialize};
use tower_lsp::lsp_types::Url;
use source_span::Span;
use std::collections::BTreeMap;

// --- 2.2 ExplainRequest ---

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExplainRequest {
    pub uri: Url,
    pub target: ExplainTarget,
    pub limits: Option<ExplainLimits>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind", content = "payload")]
#[serde(rename_all = "camelCase")]
pub enum ExplainTarget {
    Goal { goal_id: String },
    Constraint { constraint_id: String },
    Meta { meta_id: String },
    Span { span: Span },
    TraceNode { trace_id: String },
    WhyBlocked { goal_id: String },
    WhyInconsistent { goal_id: String },
}

// --- 2.3 ExplainLimits ---

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExplainLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_children_per_node: usize,
    pub max_label_chars: usize,
    pub timeout_ms: u64,
}

impl Default for ExplainLimits {
    fn default() -> Self {
        Self {
            max_nodes: 100,
            max_depth: 50, // Matches oracle spec suggestion
            max_children_per_node: 30,
            max_label_chars: 2000,
            timeout_ms: 250,
        }
    }
}

// --- 3.1 ExplanationView ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationView {
    pub root: ExplanationNode,
    pub total_nodes: usize,
    pub truncated: bool,
    pub truncation_reason: Option<String>,
    pub traversal: String, // Fixed to "bfs"
}

// --- 3.2 ExplanationNode ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationNode {
    pub id: String,
    pub kind: ExplanationKind,
    pub label: String,
    pub jump_target: Option<Span>,
    pub children: Vec<ExplanationNode>,
    pub metadata: BTreeMap<String, String>, // Stable ordering
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub enum ExplanationKind {
    GoalEmission = 0,
    Constraint = 1,
    MetaDependency = 2,
    RuleApplication = 3,
    BlockerChain = 4,
    Conflict = 5,
    Derived = 6,
    Blocked = 7, // Added for root of blocked explainer
}

/// Helper to validate spans against text length.
pub fn validate_span(span: Span, text_len: usize) -> Option<Span> {
    if span.start > span.end || span.end > text_len {
        None
    } else {
        Some(span)
    }
}
