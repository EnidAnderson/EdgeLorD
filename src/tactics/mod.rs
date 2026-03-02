//! Tactics layer — canonical entry point for tactic modules.
//!
//! The live tactic system lives in:
//! - `view.rs`          — trait `Tactic`, `TacticRequest`, `TacticResult`, `TacticAction`
//! - `registry.rs`      — `TacticRegistry` (deterministic `BTreeMap`-based, INV D-*)
//! - `query.rs`         — `TacticQuery` / `SemanticQuery` (goal-at-cursor, blockers)
//! - `edit.rs`          — edit helpers
//! - `rule_index.rs`    — SC0: doctrine-aware rule index
//! - `speculative.rs`   — SC1: dry-run rule applicability
//! - `kernel_adapter.rs`— SC4: bridge between `KernelTactic` and EdgeLorD `Tactic`
//! - `stdlib/`          — standard-library tactic implementations

pub mod view;
pub mod registry;
pub mod edit;
pub mod query;
pub mod rule_index;
pub mod speculative;
pub mod kernel_adapter;
pub mod stdlib;
