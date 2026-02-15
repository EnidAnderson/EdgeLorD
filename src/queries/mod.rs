/// Phase 1.2B: DB-Native Named Queries
///
/// This module defines canonical queries for incremental computation with SniperDB.
/// Each query has:
/// - CompileInput{V}: Canonical, deterministically serialized input
/// - Query signature: Fully determined by input hash
/// - Output artifact: Compilation result with proof of soundness
///
/// Hard invariant: If input matches, output matches (deterministic, pure).

pub mod check_unit;

pub use check_unit::{CompileInputV1, Q_CHECK_UNIT_V1, DiagnosticsArtifactV1};
