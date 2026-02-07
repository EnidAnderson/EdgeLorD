//! Probe trait and core probe types.
//!
//! A probe doctrine is a small semantic world we can map into and evaluate.
//! This is the Mac Lane-native approach: interpretations are first-class objects.

use serde::{Deserialize, Serialize};
use crate::refute::types::{BoundedList, RefuteLimits, RefuteFragment, TruncationReason};
use crate::refute::slice::RefuteSlice;
use crate::refute::witness::FailureWitness;

// ============================================================================
// Probe Key (stable identity)
// ============================================================================

/// Stable identity for a probe doctrine.
///
/// **Mac Lane move**: includes deformation level and semantic description
/// for education + ML training data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeKey {
    /// Unique identifier (e.g., "rewrite_obstruction", "finite_cat_3").
    pub id: String,
    /// Maximum coherence level this probe supports.
    pub level: u8,
    /// Human-readable description of what semantic principle is being tested.
    pub description: String,
}

// ============================================================================
// Interpretation Candidate
// ============================================================================

/// A candidate interpretation from a probe doctrine.
///
/// Stored structurally; rendered via PrettyCtx at the boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterpretationCandidate {
    /// Domain size (for finite category probes).
    pub domain_size: usize,
    /// Interpretation ID (probe-specific, deterministic).
    pub id: String,
    /// Structural data for the interpretation (probe-specific).
    pub data: InterpretationData,
}

/// Probe-specific interpretation data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InterpretationData {
    /// Rewrite probe: no domain, just rewrite state.
    Rewrite,
    /// Finite category: objects and morphism assignments.
    FiniteCategory {
        objects: usize,
        morphism_count: usize,
    },
}

// ============================================================================
// Probe Check Result (honest semantics)
// ============================================================================

/// Result of checking an interpretation.
///
/// **Critical invariant**: `NoFailureFoundWithinBounds` is NOT a counterexample.
/// Only `FoundFailure` with a witness counts as refutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefuteCheckResult {
    /// Found a genuine failure with witness.
    FoundFailure(FailureWitness),
    /// Could not find failure within resource bounds (NOT a counterexample).
    NoFailureFoundWithinBounds { reason: TruncationReason },
    /// Fragment not supported by this probe (honest decline).
    UnsupportedFragment { reason: String },
}

// ============================================================================
// Probe Doctrine Trait
// ============================================================================

/// A probe doctrine for Mac Lane-native refutation.
///
/// Probes enumerate interpretations and check whether obligations hold.
/// This is doctrine-agnostic: the same interface works for rewrite probes,
/// finite category probes, and (eventually) SMT probes.
pub trait ProbeDoctrine: Send + Sync {
    /// Stable identity for this probe.
    fn key(&self) -> ProbeKey;

    /// Maximum coherence level this probe supports.
    fn max_level(&self) -> u8;

    /// Whether this probe can handle the given fragment.
    fn supports_fragment(&self, frag: &RefuteFragment) -> bool;

    /// Enumerate candidate interpretations (bounded, deterministic).
    fn enumerate_interpretations(
        &self,
        slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> BoundedList<InterpretationCandidate>;

    /// Check whether obligations hold under this interpretation.
    ///
    /// **Invariant**: Must return `NoFailureFoundWithinBounds` on timeout,
    /// not `FoundFailure`. Only genuine obstructions yield `FoundFailure`.
    fn check(
        &self,
        interp: &InterpretationCandidate,
        slice: &RefuteSlice,
        limits: &RefuteLimits,
    ) -> RefuteCheckResult;
}
