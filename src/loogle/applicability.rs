/// Applicability engine that checks if a lemma can unify with a goal
use crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary, ReconstructionPlan};
use super::LoogleResult;

/// Result of checking applicability of a lemma to a goal
#[derive(Debug, Clone)]
pub struct ApplicabilityResult {
    pub applicable: bool,
    pub confidence: f32,  // 0.0 to 1.0
    pub unification_preview: Option<String>,  // e.g., "?A := Nat, ?B := List Nat"
    pub pedagogical_rationale: String,
}

/// Check if a lemma result is applicable to a current goal
pub fn check_applicability(
    lemma: &LoogleResult,
    goal_fingerprint: &str,
) -> ApplicabilityResult {
    // Phase 2 MVP: Simple string matching
    // TODO: Implement proper unification algorithm with meta-variable substitution
    
    let applicable = lemma_matches_goal(lemma, goal_fingerprint);
    let confidence = if applicable { 0.8 } else { 0.0 };
    
    let unification_preview = if applicable {
        Some("Exact match (unification TBD)".to_string())
    } else {
        None
    };
    
    let pedagogical_rationale = if applicable {
        format!(
            "Lemma '{}' applies because its structure matches your goal. {}",
            lemma.name,
            lemma.doc
        )
    } else {
        format!("Lemma '{}' does not apply to this goal", lemma.name)
    };
    
    ApplicabilityResult {
        applicable,
        confidence,
        unification_preview,
        pedagogical_rationale,
    }
}

/// Simple string-based matching for MVP
/// TODO: Replace with proper structural unification
fn lemma_matches_goal(lemma: &LoogleResult, goal_fp: &str) -> bool {
    // For now, just check if the lemma's rationale contains the goal fingerprint
    // This is a placeholder for proper unification
    lemma.rationale.contains(goal_fp) || goal_fp.contains(&lemma.name)
}

/// Convert a LoogleResult with applicability check into a Proposal
pub fn to_proposal(
    lemma: LoogleResult,
    applicability: ApplicabilityResult,
    anchor: String,
) -> Proposal<LemmaPayload> {
    let id = format!("loogle_{}", uuid::Uuid::new_v4());
    
    Proposal {
        id,
        anchor,
        kind: ProposalKind::Lemma,
        payload: LemmaPayload {
            name: lemma.name.clone(),
            doc: lemma.doc.clone(),
            unification_preview: applicability.unification_preview.clone(),
        },
        evidence: EvidenceSummary {
            rationale: applicability.pedagogical_rationale,
            trace_nodes: vec![], // TODO: Add provenance trace
        },
        status: ProposalStatus::Advisory,  // Always advisory until kernel-checked
        reconstruction: Some(ReconstructionPlan {
            engine: "manual".to_string(),
            steps: vec![
                format!("apply_lemma {}", lemma.name),
                "verify_result".to_string(),
            ],
        }),
        score: applicability.confidence,
        truncated: false,
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LemmaPayload {
    pub name: String,
    pub doc: String,
    pub unification_preview: Option<String>,
}
