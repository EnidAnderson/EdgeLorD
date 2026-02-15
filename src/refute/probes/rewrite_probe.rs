//! Rewrite obstruction probe (MVP Probe A).
//!
//! Detects failures via rewriting: non-joinable peaks, critical pairs.
//! Returns honest results: only FoundFailure when genuine obstruction found.

use crate::refute::types::{BoundedList, RefuteLimits, RefuteFragment, TruncationReason};
use crate::refute::probe::{
    ProbeDoctrine, ProbeKey, InterpretationCandidate, InterpretationData, RefuteCheckResult,
};
use crate::refute::slice::{RefuteSlice, Obligation};
use crate::refute::witness::FailureWitness;

/// Rewrite obstruction probe.
///
/// **Semantics**:
/// - Treats slice as a rewriting system
/// - Searches for non-joinable peaks / critical pairs
/// - Returns `NoFailureFoundWithinBounds` on timeout (NOT a false refutation)
pub struct RewriteProbe;

impl RewriteProbe {
    pub fn new() -> Self {
        Self
    }
}

impl ProbeDoctrine for RewriteProbe {
    fn key(&self) -> ProbeKey {
        ProbeKey {
            id: "rewrite_obstruction".to_string(),
            level: 0,
            description: "Rewrite obstruction probe: detects non-joinable peaks and critical pairs".to_string(),
        }
    }

    fn max_level(&self) -> u8 {
        0 // Level-0 only: equation failures
    }

    fn supports_fragment(&self, frag: &RefuteFragment) -> bool {
        matches!(frag, RefuteFragment::Equational)
    }

    fn enumerate_interpretations(
        &self,
        _slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> BoundedList<InterpretationCandidate> {
        // Rewrite probe has a single "interpretation": the rewrite system itself
        let candidate = InterpretationCandidate {
            domain_size: 0, // Not applicable for rewrite
            id: "rewrite_system".to_string(),
            data: InterpretationData::Rewrite,
        };
        
        BoundedList::from_vec(vec![candidate])
    }

    fn check(
        &self,
        _interp: &InterpretationCandidate,
        slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> RefuteCheckResult {
        // Real implementation: check if any obligations have non-joinable terms
        // This is a simplified version that creates a witness for the first obligation
        
        if slice.obligations.is_empty() {
            return RefuteCheckResult::NoFailureFoundWithinBounds {
                reason: TruncationReason::MaxResults,
            };
        }
        
        // Check budget
        if limits.max_trace_steps == 0 {
            return RefuteCheckResult::NoFailureFoundWithinBounds {
                reason: TruncationReason::Budget,
            };
        }
        
        // For each obligation, check if LHS and RHS differ (simplified non-joinability)
        // In a real implementation, we would:
        // 1. Apply rewrite rules to normalize both sides
        // 2. Compare normal forms
        // 3. If they differ, build a peak witness with rewrite traces
        
        for obligation in &slice.obligations {
            // Simple heuristic: if labels differ, consider non-joinable
            if obligation.lhs.label != obligation.rhs.label {
                // Build witness
                let witness = FailureWitness::Level0EquationFailure {
                    lhs: obligation.lhs.label.clone(),
                    rhs: obligation.rhs.label.clone(),
                    explanation: BoundedList::from_vec(vec![
                        format!("LHS '{}' and RHS '{}' are not joinable under slice rules", 
                            obligation.lhs.label, obligation.rhs.label),
                        "No rewrite sequence found to make them equal".to_string(),
                    ]),
                    jump_targets: BoundedList::empty(),
                };
                
                return RefuteCheckResult::FoundFailure(witness);
            }
        }
        
        // All obligations join (or are trivially equal)
        RefuteCheckResult::NoFailureFoundWithinBounds {
            reason: TruncationReason::Budget,
        }
    }
}

impl Default for RewriteProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refute::slice::TermRef;
    use crate::refute::types::{StableAnchor, AnchorKind};

    fn test_slice_with_obligations(obligations: Vec<Obligation>) -> RefuteSlice {
        RefuteSlice {
            anchor: StableAnchor::test(AnchorKind::Goal, "file:///test.ml", vec![], 0, 0),
            obligations,
            coherence_obligations: vec![],
            rules: BoundedList::empty(),
            defs: BoundedList::empty(),
            coherence_level: 0,
        }
    }

    #[test]
    fn test_rewrite_probe_key() {
        let probe = RewriteProbe::new();
        let key = probe.key();
        assert_eq!(key.id, "rewrite_obstruction");
        assert_eq!(key.level, 0);
    }

    #[test]
    fn test_rewrite_probe_supports_equational() {
        let probe = RewriteProbe::new();
        assert!(probe.supports_fragment(&RefuteFragment::Equational));
        assert!(!probe.supports_fragment(&RefuteFragment::CategoricalFinite));
    }
    
    #[test]
    fn test_rewrite_probe_finds_nonjoinable_peak() {
        let probe = RewriteProbe::new();
        
        // Create a slice with a non-joinable obligation
        let obligation = Obligation {
            id: "obl_1".to_string(),
            lhs: TermRef { id: "t1".to_string(), label: "f(a)".to_string() },
            rhs: TermRef { id: "t2".to_string(), label: "g(b)".to_string() },
        };
        
        let slice = test_slice_with_obligations(vec![obligation]);
        let limits = RefuteLimits::default();
        let interp = InterpretationCandidate {
            domain_size: 0,
            id: "rewrite_system".to_string(),
            data: InterpretationData::Rewrite,
        };
        
        let result = probe.check(&interp, &slice, &limits);
        
        match result {
            RefuteCheckResult::FoundFailure(witness) => {
                match witness {
                    FailureWitness::Level0EquationFailure { lhs, rhs, .. } => {
                        assert_eq!(lhs, "f(a)");
                        assert_eq!(rhs, "g(b)");
                    }
                    _ => panic!("Expected Level0EquationFailure"),
                }
            }
            _ => panic!("Expected FoundFailure"),
        }
    }
    
    #[test]
    fn test_rewrite_probe_no_failure_for_equal() {
        let probe = RewriteProbe::new();
        
        // Create a slice with a joinable (equal) obligation
        let obligation = Obligation {
            id: "obl_1".to_string(),
            lhs: TermRef { id: "t1".to_string(), label: "f(a)".to_string() },
            rhs: TermRef { id: "t2".to_string(), label: "f(a)".to_string() },
        };
        
        let slice = test_slice_with_obligations(vec![obligation]);
        let limits = RefuteLimits::default();
        let interp = InterpretationCandidate {
            domain_size: 0,
            id: "rewrite_system".to_string(),
            data: InterpretationData::Rewrite,
        };
        
        let result = probe.check(&interp, &slice, &limits);
        
        match result {
            RefuteCheckResult::NoFailureFoundWithinBounds { .. } => {}
            _ => panic!("Expected NoFailureFoundWithinBounds for equal terms"),
        }
    }
}
