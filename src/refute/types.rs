//! Core types for Mac Lane-native refutation engine.
//!
//! These types follow the universal bounded list schema and use structured
//! internal representations (no pre-rendered strings).

use serde::{Deserialize, Serialize};

// ============================================================================
// StableAnchor (local copy for independence from deep dependency paths)
// ============================================================================

/// A stable identifier for a proof artifact.
///
/// This is a local copy to avoid deep dependency paths.
/// Matches the structure in `new_surface_syntax::diagnostics::anchors`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct StableAnchor {
    pub kind: AnchorKind,
    pub file_uri: String,
    pub owner_path: Vec<String>,
    pub ordinal: u32,
    pub span_fingerprint: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub enum AnchorKind { 
    Goal, 
    Constraint, 
    Binding, 
    AstNode,
    Hole,
}

impl StableAnchor {
    /// Compute the deterministic string ID.
    pub fn to_id_string(&self) -> String {
        let path = if self.owner_path.is_empty() {
            "root".to_string()
        } else {
            self.owner_path.join("/")
        };
        format!("{:?}:{}:{}:{}", self.kind, self.file_uri, path, self.ordinal)
    }
    
    /// Create a test anchor.
    pub fn test(kind: AnchorKind, file_uri: &str, owner_path: Vec<String>, ordinal: u32, span_fingerprint: u64) -> Self {
        Self {
            kind,
            file_uri: file_uri.to_string(),
            owner_path,
            ordinal,
            span_fingerprint,
        }
    }
}

// ============================================================================
// Bounded List (universal schema)
// ============================================================================

/// A bounded list with full truncation tracking.
///
/// **Invariant**: This is the universal bounded list type used across
/// EdgeLorD (refute, explain, loogle). Always includes:
/// - `total_count`: how many items existed before capping
/// - `truncation_reason`: why we stopped (if truncated)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct BoundedList<T> {
    pub items: Vec<T>,
    pub total_count: usize,
    pub truncated: bool,
    pub truncation_reason: Option<TruncationReason>,
}

impl<T> BoundedList<T> {
    /// Create a non-truncated list.
    pub fn from_vec(items: Vec<T>) -> Self {
        let count = items.len();
        Self {
            items,
            total_count: count,
            truncated: false,
            truncation_reason: None,
        }
    }

    /// Create a truncated list.
    pub fn truncated(items: Vec<T>, total_count: usize, reason: TruncationReason) -> Self {
        Self {
            items,
            total_count,
            truncated: true,
            truncation_reason: Some(reason),
        }
    }

    /// Create an empty list.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total_count: 0,
            truncated: false,
            truncation_reason: None,
        }
    }
}

impl<T> Default for BoundedList<T> {
    fn default() -> Self {
        Self::empty()
    }
}

/// Reason for truncation in bounded operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TruncationReason {
    Timeout,
    MaxResults,
    MaxDepth,
    Budget,
}

// ============================================================================
// Decision Info (uniform envelope for witness decidability)
// ============================================================================

/// Uniform decision envelope for every witness.
///
/// This prevents clients from having to interpret enum variants as policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionInfo {
    /// Whether this fragment is decidable by this probe.
    pub decidable: bool,
    /// Whether a decision was actually reached (failure found).
    pub decided: bool,
    /// Reason if not decidable or not decided.
    pub reason: Option<String>,
}

impl DecisionInfo {
    /// Decided failure within decidable fragment.
    pub fn decided() -> Self {
        Self { decidable: true, decided: true, reason: None }
    }
    
    /// Decidable but no failure found.
    pub fn not_found(reason: &str) -> Self {
        Self { decidable: true, decided: false, reason: Some(reason.to_string()) }
    }
    
    /// Not decidable by this probe.
    pub fn undecidable(reason: &str) -> Self {
        Self { decidable: false, decided: false, reason: Some(reason.to_string()) }
    }
}

// ============================================================================
// Jump Target (structured span + anchor + label)
// ============================================================================

/// A structured jump target for explain/UI integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpTarget {
    /// Stable anchor string (if available).
    pub anchor: Option<String>,
    /// Byte span (converted to UTF-16 at LSP boundary).
    pub span: Option<ByteSpan>,
    /// Human-readable label.
    pub label: Option<String>,
    /// Kind of target (e.g., "lhs", "rhs", "redex", "ruleSite").
    pub kind: Option<String>,
}

impl JumpTarget {
    pub fn from_span(start: usize, end: usize) -> Self {
        Self {
            anchor: None,
            span: Some(ByteSpan { start, end }),
            label: None,
            kind: None,
        }
    }
    
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }
    
    pub fn with_kind(mut self, kind: &str) -> Self {
        self.kind = Some(kind.to_string());
        self
    }
}

/// Byte span for jump targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

// ============================================================================
// Refutation Limits
// ============================================================================

/// Resource limits for refutation.
///
/// Defaults are conservative for fast p95 response times.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefuteLimits {
    /// Maximum interpretation candidates to enumerate per probe.
    pub max_interpretations: usize,
    /// Maximum domain size for finite category probes (default: 3).
    pub max_domain_size: usize,
    /// Maximum trace steps for slice extraction.
    pub max_trace_steps: usize,
    /// Maximum coherence level (0=equation, 1=2-cell, 2=higher).
    pub max_coherence_level: u8,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
}

impl Default for RefuteLimits {
    fn default() -> Self {
        Self {
            max_interpretations: 50,
            max_domain_size: 3,      // Conservative; can configure up to 5
            max_trace_steps: 100,
            max_coherence_level: 0,  // MVP: level-0 only
            timeout_ms: 500,
        }
    }
}

// ============================================================================
// Fragment Classification
// ============================================================================

/// Fragment of theory that a probe can handle.
///
/// Used for "honest unsupported" - probes decline fragments they can't check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefuteFragment {
    /// Pure equational fragment (no higher structure).
    Equational,
    /// Finite categorical structure (composition, identities).
    CategoricalFinite,
    /// Higher coherence at specified level.
    HigherCoherence(u8),
}

impl RefuteFragment {
    /// The coherence level this fragment requires.
    pub fn level(&self) -> u8 {
        match self {
            RefuteFragment::Equational => 0,
            RefuteFragment::CategoricalFinite => 0,
            RefuteFragment::HigherCoherence(n) => *n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_list_from_vec() {
        let list: BoundedList<i32> = BoundedList::from_vec(vec![1, 2, 3]);
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.total_count, 3);
        assert!(!list.truncated);
        assert!(list.truncation_reason.is_none());
    }

    #[test]
    fn test_bounded_list_truncated() {
        let list: BoundedList<i32> = BoundedList::truncated(
            vec![1, 2],
            100,
            TruncationReason::MaxResults,
        );
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.total_count, 100);
        assert!(list.truncated);
        assert_eq!(list.truncation_reason, Some(TruncationReason::MaxResults));
    }

    #[test]
    fn test_refute_limits_default() {
        let limits = RefuteLimits::default();
        assert_eq!(limits.max_domain_size, 3);
        assert_eq!(limits.max_coherence_level, 0);
    }
}
