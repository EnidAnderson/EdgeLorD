//! LSP handler for edgelord/refute endpoint.
//!
//! This module provides the stable JSON contract for refutation.

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary};
use crate::refute::types::{BoundedList, RefuteLimits, StableAnchor, AnchorKind, DecisionInfo};
use crate::refute::witness::{CounterexamplePayload, Counterexample, FailureWitness, InterpretationSummary, InterpretationKind, DiagramBoundary};
use crate::refute::probe::ProbeKey;
use crate::refute::slice::SliceSummary;

// ============================================================================
// Request/Response Schema (stable JSON contract)
// ============================================================================

/// Version of the refute protocol. Bump on breaking changes.
pub const REFUTE_PROTOCOL_VERSION: u32 = 1;

/// Request for refutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefuteRequest {
    /// Target anchor ID string.
    pub anchor: String,
    /// Requested coherence level (0=equations, 1=2-cells, 2=higher).
    pub coherence_level: u8,
    /// Optional resource limits.
    #[serde(default)]
    pub limits: Option<RefuteLimits>,
}

/// Response envelope for refutation.
///
/// **Invariant**: Field names and ordering are stable. Bump `version` on breaking changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefuteResponse {
    /// Protocol version (always 1 until breaking change).
    pub version: u32,
    /// Timestamp in milliseconds (0 in test mode for determinism).
    pub timestamp_ms: u64,
    /// List of proposals (bounded, deterministically ordered).
    pub proposals: BoundedList<Proposal<CounterexamplePayload>>,
    /// Request metadata (for debugging/truncation explanation).
    pub meta: RefuteMeta,
}

/// Metadata about the refutation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefuteMeta {
    /// Anchor string echoed back.
    pub anchor: String,
    /// Coherence level requested.
    pub coherence_level_requested: u8,
    /// Limits used (echoed for truncation explanation).
    pub limits: RefuteLimits,
    /// Engine identifier.
    pub engine: String,
}

// ============================================================================
// Deterministic Ordering
// ============================================================================

/// Sort proposals deterministically.
///
/// Order by: status, level, probe, score (desc), failure fingerprint, id
pub fn sort_proposals(proposals: &mut [Proposal<CounterexamplePayload>]) {
    proposals.sort_by(|a, b| {
        // 1. Status: Certified before Advisory
        let status_cmp = status_rank(&a.status).cmp(&status_rank(&b.status));
        if status_cmp != std::cmp::Ordering::Equal {
            return status_cmp;
        }
        
        // 2. Level ascending
        let level_cmp = a.payload.counterexample.level.cmp(&b.payload.counterexample.level);
        if level_cmp != std::cmp::Ordering::Equal {
            return level_cmp;
        }
        
        // 3. Probe lexicographic
        let probe_cmp = a.payload.counterexample.probe.id.cmp(&b.payload.counterexample.probe.id);
        if probe_cmp != std::cmp::Ordering::Equal {
            return probe_cmp;
        }
        
        // 4. Score descending
        let score_cmp = b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal);
        if score_cmp != std::cmp::Ordering::Equal {
            return score_cmp;
        }
        
        // 5. Failure fingerprint
        let fp_a = failure_fingerprint(&a.payload.counterexample.failure);
        let fp_b = failure_fingerprint(&b.payload.counterexample.failure);
        let fp_cmp = fp_a.cmp(&fp_b);
        if fp_cmp != std::cmp::Ordering::Equal {
            return fp_cmp;
        }
        
        // 6. ID lexicographic
        a.id.cmp(&b.id)
    });
}

fn status_rank(status: &ProposalStatus) -> u8 {
    match status {
        ProposalStatus::Certified => 0,
        ProposalStatus::Advisory => 1,
    }
}

/// Compute fingerprint for a failure witness.
fn failure_fingerprint(witness: &FailureWitness) -> String {
    let mut hasher = Sha256::new();
    match witness {
        FailureWitness::Level0EquationFailure { lhs, rhs, .. } => {
            hasher.update(b"L0:");
            hasher.update(lhs.as_bytes());
            hasher.update(b"=");
            hasher.update(rhs.as_bytes());
        }
        FailureWitness::Level1CoherenceMissing { boundary, diagram, .. } => {
            hasher.update(b"L1:");
            hasher.update(boundary.source_path.as_bytes());
            hasher.update(b"=>");
            hasher.update(boundary.target_path.as_bytes());
            if let Some(d) = diagram {
                hasher.update(d.as_bytes());
            }
        }
        FailureWitness::Level2HigherCoherenceFailure { expected_higher_cell, .. } => {
            hasher.update(b"L2:");
            hasher.update(expected_higher_cell.as_bytes());
        }
        FailureWitness::UnsupportedButSuspicious { reason } => {
            hasher.update(b"UNS:");
            hasher.update(reason.as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// Proposal ID Generation (deterministic)
// ============================================================================

/// Generate deterministic proposal ID from (anchor, probe, witness fingerprint).
pub fn generate_proposal_id(anchor: &str, probe: &ProbeKey, witness: &FailureWitness) -> String {
    let mut hasher = Sha256::new();
    hasher.update(anchor.as_bytes());
    hasher.update(b":");
    hasher.update(probe.id.as_bytes());
    hasher.update(b":");
    hasher.update(failure_fingerprint(witness).as_bytes());
    let hash = hasher.finalize();
    format!("refute:{}", &hash[..8].iter().map(|b| format!("{:02x}", b)).collect::<String>())
}

// ============================================================================
// Handle Refute Request
// ============================================================================

/// Handle a refute request and return a stable response.
///
/// **Invariant**: Output is byte-for-byte deterministic for same input.
pub fn handle_refute_request(
    req: RefuteRequest,
    test_mode: bool,
) -> RefuteResponse {
    let limits = req.limits.clone().unwrap_or_default();
    let timestamp = if test_mode { 0 } else { current_timestamp_ms() };
    
    // Parse anchor (MVP: create from string)
    let anchor = StableAnchor {
        kind: AnchorKind::Goal,
        file_uri: "unknown".to_string(),
        owner_path: vec![],
        ordinal: 0,
        span_fingerprint: 0,
    };
    
    // Extract slice and run refuter (MVP: empty results)
    let mut proposals = Vec::new();
    
    // TODO: Wire to actual ProofState when available
    // For now, return a placeholder "no counterexample" response
    if req.coherence_level >= 1 {
        // Return MissingCoherenceWitness for level ≥ 1
        proposals.push(create_missing_coherence_proposal(
            &req.anchor,
            req.coherence_level,
        ));
    }
    
    // Sort deterministically
    sort_proposals(&mut proposals);
    
    // Round scores for JSON stability
    for p in &mut proposals {
        p.score = round_score(p.score);
    }
    
    RefuteResponse {
        version: REFUTE_PROTOCOL_VERSION,
        timestamp_ms: timestamp,
        proposals: BoundedList::from_vec(proposals),
        meta: RefuteMeta {
            anchor: req.anchor,
            coherence_level_requested: req.coherence_level,
            limits,
            engine: "refute_v1".to_string(),
        },
    }
}

/// Create a MissingCoherenceWitness proposal for level ≥ 1.
fn create_missing_coherence_proposal(
    anchor: &str,
    level: u8,
) -> Proposal<CounterexamplePayload> {
    let probe = ProbeKey {
        id: "coherence_check".to_string(),
        level,
        description: format!("Coherence level {} obligation", level),
    };
    
    let boundary = DiagramBoundary {
        source_path: "source".to_string(),
        target_path: "target".to_string(),
        required_cell: format!("2-cell α : source ⇒ target at level {}", level),
        anchor: None,
    };
    
    let witness = FailureWitness::Level1CoherenceMissing {
        boundary: boundary.clone(),
        diagram: Some("source ⟶ target".to_string()),
        expected_2cell: format!("2-cell α : source ⇒ target at level {}", level),
        reason: format!(
            "Probe only reasons at level 0. Cannot decide level {} coherence. \
             Need a 2-cell filler for this diagram boundary.",
            level
        ),
    };
    
    let id = generate_proposal_id(anchor, &probe, &witness);
    
    Proposal {
        id,
        anchor: anchor.to_string(),
        kind: ProposalKind::Refutation,
        payload: CounterexamplePayload::new(Counterexample {
            probe: probe.clone(),
            interpretation: InterpretationSummary {
                kind: InterpretationKind::Unknown { description: "N/A".to_string() },
                description: "N/A (coherence check)".to_string(),
                domain_size: None,
            },
            failure: witness,
            level,
            slice_summary: SliceSummary {
                obligations_included: 0,
                rules_included: 0,
                defs_included: 0,
                trace_steps_used: 0,
                truncated: false,
                reason: None,
            },
            decision: DecisionInfo::undecidable(
                &format!("Coherence level {} not implemented", level)
            ),
        }),
        evidence: EvidenceSummary {
            rationale: format!("Cannot decide coherence at level {}. Missing higher cell.", level),
            trace_nodes: vec![],
        },
        status: ProposalStatus::Advisory,
        reconstruction: None,
        score: 0.0, // No evidence of failure
        truncated: false,
    }
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn round_score(score: f32) -> f32 {
    (score * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_proposal_id_deterministic() {
        let probe = ProbeKey {
            id: "test_probe".to_string(),
            level: 0,
            description: "Test".to_string(),
        };
        let witness = FailureWitness::Level0EquationFailure {
            lhs: "a".to_string(),
            rhs: "b".to_string(),
            explanation: BoundedList::empty(),
            jump_targets: BoundedList::empty(),
        };
        
        let id1 = generate_proposal_id("anchor1", &probe, &witness);
        let id2 = generate_proposal_id("anchor1", &probe, &witness);
        assert_eq!(id1, id2);
        
        // Different anchor → different ID
        let id3 = generate_proposal_id("anchor2", &probe, &witness);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_handle_refute_deterministic() {
        let req = RefuteRequest {
            anchor: "goal:test:foo:0".to_string(),
            coherence_level: 0,
            limits: None,
        };
        
        let resp1 = handle_refute_request(req.clone(), true);
        let resp2 = handle_refute_request(req, true);
        
        // Serialize and compare
        let json1 = serde_json::to_string_pretty(&resp1).unwrap();
        let json2 = serde_json::to_string_pretty(&resp2).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_coherence_level_1_returns_missing_witness() {
        let req = RefuteRequest {
            anchor: "goal:test:foo:0".to_string(),
            coherence_level: 1,
            limits: None,
        };
        
        let resp = handle_refute_request(req, true);
        assert_eq!(resp.proposals.items.len(), 1);
        
        let proposal = &resp.proposals.items[0];
        match &proposal.payload.counterexample.failure {
            FailureWitness::Level1CoherenceMissing { .. } => {}
            _ => panic!("Expected Level1CoherenceMissing"),
        }
    }
}
