//! Tactics layer — canonical entry point for tactic modules.
//!
//! The live tactic system lives in:
//! - `view.rs`     — trait `Tactic`, `TacticRequest`, `TacticResult`, `TacticAction`
//! - `registry.rs` — `TacticRegistry` (deterministic `BTreeMap`-based, INV D-*)
//! - `query.rs`    — `TacticQuery` / `SemanticQuery` (goal-at-cursor, blockers)
//! - `edit.rs`     — edit helpers
//! - `stdlib/`     — standard-library tactic implementations

pub mod view;
pub mod registry;
pub mod edit;
pub mod query;
pub mod stdlib;
