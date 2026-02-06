use serde::{Serialize, Deserialize};

/// A universal protocol for knowledge-sharing suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal<T> {
    pub id: String,                 // Session-unique ID
    pub anchor: String,             // StableAnchor ID string
    pub kind: ProposalKind,
    pub payload: T,
    pub evidence: EvidenceSummary,
    pub status: ProposalStatus,
    pub reconstruction: Option<ReconstructionPlan>,
    pub score: f32,                 // 0.0 to 1.0
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    Rewrite,
    Lemma,
    Tactic,
    Refutation,
    Hint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Certified,
    Advisory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSummary {
    pub rationale: String,
    pub trace_nodes: Vec<String>, // Bounded IDs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructionPlan {
    pub engine: String, // "egg", "manual", others
    pub steps: Vec<String>,
}
