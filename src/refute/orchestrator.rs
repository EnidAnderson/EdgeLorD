//! Refuter orchestrator.
//!
//! Tries probes in deterministic priority order and returns the
//! first valid counterexample as a Proposal.

use std::sync::Arc;
use crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary};
use crate::refute::types::{RefuteLimits, StableAnchor, DecisionInfo};
use crate::refute::probe::{ProbeDoctrine, ProbeKey, RefuteCheckResult, InterpretationData};
use crate::refute::slice::{RefuteSlice, SliceSummary};
use crate::refute::witness::{CounterexamplePayload, Counterexample, InterpretationSummary, InterpretationKind, FailureWitness};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

// ============================================================================
// Refute Request
// ============================================================================

/// Request for refutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefuteRequest {
    /// Target anchor to refute.
    pub target: String,  // StableAnchor ID string at LSP boundary
    /// Resource limits.
    pub limits: Option<RefuteLimits>,
}

// ============================================================================
// Refuter
// ============================================================================

/// Main refuter orchestrator.
///
/// **Invariants**:
/// - Deterministic probe ordering
/// - Returns Proposal<CounterexamplePayload>, not bespoke result
/// - ProposalStatus::Advisory always for Phase 10
pub struct Refuter {
    probes: Vec<Arc<dyn ProbeDoctrine>>,
}

impl Refuter {
    /// Create a new refuter with the given probes.
    pub fn new(probes: Vec<Arc<dyn ProbeDoctrine>>) -> Self {
        Self { probes }
    }

    /// Create a refuter with default MVP probes.
    pub fn with_default_probes() -> Self {
        use crate::refute::probes::rewrite_probe::RewriteProbe;
        
        Self::new(vec![
            Arc::new(RewriteProbe::new()),
            // FiniteCatProbe added in step 2
        ])
    }

    /// Run refutation and return a Proposal.
    ///
    /// **Semantics**:
    /// - Tries probes in deterministic order
    /// - Returns first counterexample found
    /// - Returns "no counterexample within bounds" if nothing found
    pub fn refute(
        &self,
        slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> Proposal<CounterexamplePayload> {
        // Try each probe in order
        for probe in self.probes_in_order(slice, limits) {
            let interps = probe.enumerate_interpretations(slice, limits);
            
            for interp in interps.items.iter() {
                let result = probe.check(interp, slice, limits);
                
                if let RefuteCheckResult::FoundFailure(witness) = result {
                    let interp_kind = match &interp.data {
                        InterpretationData::Rewrite => InterpretationKind::Rewrite,
                        InterpretationData::FiniteCategory { objects, morphism_count } => {
                            InterpretationKind::FiniteCategory {
                                objects: *objects,
                                morphism_count: *morphism_count,
                            }
                        }
                    };
                    
                    let counterexample = Counterexample {
                        probe: probe.key(),
                        interpretation: InterpretationSummary {
                            kind: interp_kind,
                            description: format!("Interpretation {}", interp.id),
                            domain_size: Some(interp.domain_size),
                        },
                        failure: witness,
                        level: 0, // MVP: level-0 only
                        slice_summary: SliceSummary::from_slice(slice, limits.max_trace_steps),
                        decision: DecisionInfo::decided(),
                    };
                    
                    return self.make_proposal(
                        slice,
                        CounterexamplePayload::new(counterexample),
                        "Counterexample found",
                        interps.truncated,
                    );
                }
            }
        }
        
        // No counterexample found within bounds
        self.make_no_counterexample_proposal(slice, limits)
    }

    /// Get probes in deterministic order for this slice.
    fn probes_in_order<'a>(
        &'a self,
        _slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> impl Iterator<Item = &'a Arc<dyn ProbeDoctrine>> {
        // Filter probes by coherence level and sort by priority
        self.probes.iter().filter(move |p| {
            p.max_level() <= limits.max_coherence_level || limits.max_coherence_level == 0
        })
    }

    /// Create a proposal from a counterexample.
    fn make_proposal(
        &self,
        slice: &RefuteSlice,
        payload: CounterexamplePayload,
        rationale: &str,
        truncated: bool,
    ) -> Proposal<CounterexamplePayload> {
        Proposal {
            id: self.compute_proposal_id(slice, &payload),
            anchor: slice.anchor.to_id_string(),
            kind: ProposalKind::Refutation,
            payload,
            evidence: EvidenceSummary {
                rationale: rationale.to_string(),
                trace_nodes: Vec::new(), // TODO: populate from slice
            },
            status: ProposalStatus::Advisory, // Always advisory for Phase 10
            reconstruction: None,
            score: 0.9,
            truncated,
        }
    }

    /// Create a "no counterexample found" proposal.
    fn make_no_counterexample_proposal(
        &self,
        slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> Proposal<CounterexamplePayload> {
        // Create a "not refuted" witness
        let witness = FailureWitness::UnsupportedButSuspicious {
            reason: format!(
                "No counterexample found within bounds: {} interps, domain≤{}, {} trace steps",
                limits.max_interpretations,
                limits.max_domain_size,
                limits.max_trace_steps,
            ),
        };
        
        let counterexample = Counterexample {
            probe: ProbeKey {
                id: "none".to_string(),
                level: 0,
                description: "No probe found failure".to_string(),
            },
            interpretation: InterpretationSummary {
                kind: InterpretationKind::Unknown { description: "N/A".to_string() },
                description: "N/A".to_string(),
                domain_size: None,
            },
            failure: witness,
            level: 0,
            slice_summary: SliceSummary::from_slice(slice, limits.max_trace_steps),
            decision: DecisionInfo::not_found("No failure found within bounds"),
        };
        
        Proposal {
            id: self.compute_proposal_id(slice, &CounterexamplePayload::new(counterexample.clone())),
            anchor: slice.anchor.to_id_string(),
            kind: ProposalKind::Refutation,
            payload: CounterexamplePayload::new(counterexample),
            evidence: EvidenceSummary {
                rationale: "No counterexample found within resource bounds".to_string(),
                trace_nodes: Vec::new(),
            },
            status: ProposalStatus::Advisory,
            reconstruction: None,
            score: 0.0, // No evidence of failure
            truncated: false,
        }
    }

    /// Compute deterministic proposal ID.
    fn compute_proposal_id(
        &self,
        slice: &RefuteSlice,
        payload: &CounterexamplePayload,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(slice.anchor.to_id_string().as_bytes());
        hasher.update(payload.counterexample.probe.id.as_bytes());
        hasher.update(&[payload.counterexample.level]);
        let hash = hasher.finalize();
        format!("refute:{:x}", hash)
    }
}
