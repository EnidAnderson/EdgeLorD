//! Mac Lane-native refutation engine.
//!
//! This module provides doctrine-agnostic refutation via probe doctrines.
//! Probes enumerate interpretations and check whether obligations hold.
//!
//! # Architecture
//!
//! - `types`: Core bounded types (BoundedList, RefuteLimits)
//! - `slice`: Theory slice extraction from ProofState
//! - `probe`: ProbeDoctrine trait and check results
//! - `witness`: Counterexample and FailureWitness types
//! - `orchestrator`: Refuter that tries probes in order
//! - `probes/`: MVP probe implementations
//! - `render`: Witness rendering via PrettyCtx

pub mod types;
pub mod slice;
pub mod probe;
pub mod witness;
pub mod orchestrator;
pub mod probes;
pub mod render;
pub mod lsp_handler;

// Re-exports for convenience
pub use types::{BoundedList, RefuteLimits, RefuteFragment, TruncationReason, StableAnchor, AnchorKind};
pub use slice::{RefuteSlice, SliceSummary, extract_slice};
pub use probe::{ProbeDoctrine, ProbeKey, RefuteCheckResult};
pub use witness::{Counterexample, FailureWitness, CounterexamplePayload};
pub use orchestrator::{Refuter, RefuteRequest as OrchestratorRequest};
pub use lsp_handler::{RefuteRequest, RefuteResponse, RefuteMeta, handle_refute_request, REFUTE_PROTOCOL_VERSION};
