//! Witness types for refutation failures.
//!
//! Witnesses are pedagogical artifacts, not solver dumps.
//! They're stored structurally and rendered via PrettyCtx.

use serde::{Deserialize, Serialize};
use crate::refute::types::{BoundedList, DecisionInfo, JumpTarget};
use crate::refute::probe::ProbeKey;
use crate::refute::slice::SliceSummary;

// ============================================================================
// Counterexample
// ============================================================================

/// A counterexample found by refutation.
///
/// This is the payload for `Proposal<CounterexamplePayload>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counterexample {
    /// Which probe found this.
    pub probe: ProbeKey,
    /// Summary of the interpretation that failed.
    pub interpretation: InterpretationSummary,
    /// The actual failure witness.
    pub failure: FailureWitness,
    /// Coherence level of the failure (0=equation, 1=2-cell, 2=higher).
    pub level: u8,
    /// What was included in the slice.
    pub slice_summary: SliceSummary,
    /// Uniform decision envelope.
    pub decision: DecisionInfo,
}

/// Summary of an interpretation for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterpretationSummary {
    /// Interpretation kind for forward compatibility.
    pub kind: InterpretationKind,
    /// Probe-specific description.
    pub description: String,
    /// Domain size (if applicable).
    pub domain_size: Option<usize>,
}

/// Tagged interpretation kind for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InterpretationKind {
    Rewrite,
    FiniteCategory { objects: usize, #[serde(rename = "morphismCount")] morphism_count: usize },
    /// Future-proof placeholder.
    Unknown { description: String },
}

// ============================================================================
// Failure Witness
// ============================================================================

/// A witness to a failure in the interpretation.
///
/// **Mac Lane-native**: This is not a "model" - it's a failed obligation
/// described in the language of proof objects (explainable + teachable).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FailureWitness {
    /// Level-0: equation failure (lhs ≠ rhs under interpretation).
    Level0EquationFailure {
        /// LHS of the failed equation (pretty-printed).
        lhs: String,
        /// RHS of the failed equation (pretty-printed).
        rhs: String,
        /// Step-by-step explanation of why they differ.
        explanation: BoundedList<String>,
        /// Structured jump targets for UI integration.
        jump_targets: BoundedList<JumpTarget>,
    },
    /// Level-1: coherence (2-cell) missing.
    Level1CoherenceMissing {
        /// Machine-facing diagram boundary.
        boundary: DiagramBoundary,
        /// Human-readable diagram (pretty-printed).
        diagram: Option<String>,
        /// Expected 2-cell that's missing.
        expected_2cell: String,
        /// Explanation of why it's needed.
        reason: String,
    },
    /// Level-2+: higher coherence failure (placeholder for future).
    Level2HigherCoherenceFailure {
        expected_higher_cell: String,
        reason: String,
    },
    /// Fragment not fully supported, but something suspicious found.
    UnsupportedButSuspicious {
        reason: String,
    },
}

// ============================================================================
// Diagram Boundary (machine-facing coherence structure)
// ============================================================================

/// A diagram boundary requiring a higher cell.
///
/// Machine-facing structure for UI, explain integration, and diagram workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramBoundary {
    /// Source path of the diagram (rendered).
    pub source_path: String,
    /// Target path of the diagram (rendered).
    pub target_path: String,
    /// Description of required 2-cell/n-cell.
    pub required_cell: String,
    /// Anchor to the source location (if available).
    pub anchor: Option<String>,
}

// ============================================================================
// ByteSpan (re-export from types for backward compat)
// ============================================================================

pub use crate::refute::types::ByteSpan;

// ============================================================================
// Payload for Proposal
// ============================================================================

/// Payload for `Proposal<CounterexamplePayload>`.
///
/// Wraps Counterexample with additional context for the proposal protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CounterexamplePayload {
    pub counterexample: Counterexample,
}

impl CounterexamplePayload {
    pub fn new(counterexample: Counterexample) -> Self {
        Self { counterexample }
    }
}

