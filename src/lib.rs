pub mod lsp;
pub mod document;
pub mod proof_session;
pub mod hint_engine;
pub mod span_conversion; // Added this line
pub mod goals_panel;
pub mod edgelord_pretty_ctx; // Added this line
pub mod explain;
pub mod tactics;
pub mod diff;
pub mod proposal;
pub mod loogle;
pub mod refute;
pub mod highlight;
pub mod caching; // Phase 1.1: Deterministic Snapshot Reuse
pub mod queries; // Phase 1.2B: DB-Native Named Queries
pub mod db_memo; // Phase 1.2B: DB-Native Memoization Wrapper
pub mod dependency_graph; // SD4: Cross-file proof dependency tracking
pub mod pattern_overlay;  // SE1: Pattern highlight overlay

pub use lsp::Backend;
