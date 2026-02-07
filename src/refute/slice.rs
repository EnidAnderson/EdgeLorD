//! Refutation slice extraction.
//!
//! A RefuteSlice is a minimal, trace-driven theory slice containing:
//! - Target obligation(s)
//! - Minimal blocker set
//! - Bounded neighborhood of rules/defs from trace graph

use serde::{Deserialize, Serialize};
use crate::refute::types::{BoundedList, RefuteLimits, TruncationReason, StableAnchor, AnchorKind};

// ============================================================================
// Slice Types
// ============================================================================

/// A minimal theory slice for refutation.
///
/// **Key invariants**:
/// - `anchor` is structured (not String)
/// - Obligations sorted deterministically
/// - Rules/defs bounded by trace neighborhood
#[derive(Debug, Clone)]
pub struct RefuteSlice {
    /// The target anchor (goal/constraint/meta being refuted).
    pub anchor: StableAnchor,
    /// Obligations to check (sorted, deterministic).
    pub obligations: Vec<Obligation>,
    /// Coherence obligations for level≥1.
    pub coherence_obligations: Vec<CoherenceObligation>,
    /// Rules from trace neighborhood.
    pub rules: BoundedList<RuleRef>,
    /// Definitions from trace neighborhood.
    pub defs: BoundedList<DefRef>,
    /// Requested coherence level (clamped to probe's max).
    pub coherence_level: u8,
}

/// An obligation to check in the refutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Obligation {
    /// Stable ID for the obligation.
    pub id: String,
    /// LHS term (structural, not pre-rendered).
    pub lhs: TermRef,
    /// RHS term (structural, not pre-rendered).
    pub rhs: TermRef,
}

/// Reference to a term (structural, rendered via PrettyCtx).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermRef {
    /// Stable ID for lookup.
    pub id: String,
    /// Brief label for display (can be truncated pretty-print).
    pub label: String,
}

/// Reference to a rule in the slice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRef {
    /// Stable ID.
    pub id: String,
    /// Rule name.
    pub name: String,
    /// Source span (byte offsets, converted to UTF-16 at LSP boundary).
    pub span: Option<(usize, usize)>,
}

/// Reference to a definition in the slice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefRef {
    /// Stable ID.
    pub id: String,
    /// Definition name.
    pub name: String,
}

// ============================================================================
// Coherence Obligations (G4)
// ============================================================================

/// A coherence obligation for level≥1 proofs.
///
/// When coherence level > 0, the slice includes diagram boundaries that need
/// to be checked or surfaced as "missing coherence" if undecidable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoherenceObligation {
    /// Coherence level (1=2-cell, 2=3-cell, etc.)
    pub level: u8,
    /// The diagram boundary that needs a filler.
    pub diagram: DiagramBoundary,
    /// Expected filler description (PrettyCtx-rendered).
    pub expected_filler: String,
    /// Anchor to the source location.
    pub anchor: StableAnchor,
}

/// A diagram boundary requiring a higher cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramBoundary {
    /// Source path of the diagram (rendered).
    pub source_path: String,
    /// Target path of the diagram (rendered).
    pub target_path: String,
    /// Description of required 2-cell/n-cell.
    pub required_cell: String,
}

// ============================================================================
// Slice Summary (educational)
// ============================================================================

/// Summary of what was included in the slice.
///
/// Explicitly educational: shows what was assumed for the refutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceSummary {
    pub obligations_included: usize,
    pub rules_included: usize,
    pub defs_included: usize,
    pub trace_steps_used: usize,
    pub truncated: bool,
    pub reason: Option<TruncationReason>,
}

impl SliceSummary {
    /// Create summary from a slice.
    pub fn from_slice(slice: &RefuteSlice, trace_steps: usize) -> Self {
        let truncated = slice.rules.truncated || slice.defs.truncated;
        let reason = slice.rules.truncation_reason
            .or(slice.defs.truncation_reason);
        
        Self {
            obligations_included: slice.obligations.len(),
            rules_included: slice.rules.items.len(),
            defs_included: slice.defs.items.len(),
            trace_steps_used: trace_steps,
            truncated,
            reason,
        }
    }
}

// ============================================================================
// Slice Extraction
// ============================================================================

/// Extract a minimal slice for refutation.
///
/// **Policy**:
/// - Start from target goal/constraint
/// - Include minimal blockers (from GoalsIndex)
/// - Include rules/defs from trace neighborhood up to max_trace_steps
/// - Deterministic sort everywhere
pub fn extract_slice(
    anchor: StableAnchor,
    _proof_state: &(), // TODO: Replace with actual ProofState type
    limits: &RefuteLimits,
) -> (RefuteSlice, SliceSummary) {
    // MVP: Create empty slice (will be populated when wired to ProofState)
    let slice = RefuteSlice {
        anchor,
        obligations: Vec::new(),
        coherence_obligations: Vec::new(),
        rules: BoundedList::empty(),
        defs: BoundedList::empty(),
        coherence_level: limits.max_coherence_level,
    };
    
    let summary = SliceSummary {
        obligations_included: 0,
        rules_included: 0,
        defs_included: 0,
        trace_steps_used: 0,
        truncated: false,
        reason: None,
    };
    
    (slice, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_anchor() -> StableAnchor {
        StableAnchor::test(AnchorKind::Goal, "file:///test.ml", 0)
    }

    #[test]
    fn test_extract_slice_empty() {
        let anchor = test_anchor();
        let limits = RefuteLimits::default();
        let (slice, summary) = extract_slice(anchor.clone(), &(), &limits);
        
        assert_eq!(slice.anchor, anchor);
        assert!(!summary.truncated);
    }
}
