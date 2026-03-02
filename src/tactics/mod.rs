//! Tactics layer — canonical entry point for tactic modules.
//!
//! - `view.rs`            — Tactic trait, TacticRequest, TacticResult, TacticAction
//! - `registry.rs`        — TacticRegistry (deterministic BTreeMap-based, INV D-*)
//! - `query.rs`           — TacticQuery / SemanticQuery (goal-at-cursor, blockers)
//! - `edit.rs`            — edit helpers
//! - `rule_index.rs`      — SC0: doctrine-aware rule index
//! - `speculative.rs`     — SC1: dry-run rule applicability
//! - `kernel_adapter.rs`  — SC4: bridge between KernelTactic and EdgeLorD Tactic
//! - `auto.rs`            — SD2: bounded BFS auto-tactic
//! - `strategy.rs`        — SD3: doctrine-parameterized proof strategies
//! - `pattern_find.rs`    — SE0: pattern occurrence engine
//! - `semantic_select.rs` — SE2: semantic selection protocol
//! - `multi_rewrite.rs`   — SE3: multi-site rewrite engine
//! - `applicability.rs`   — SE4: tactic applicability index
//! - `stdlib/`            — standard-library tactic implementations

pub mod view;
pub mod registry;
pub mod edit;
pub mod query;
pub mod rule_index;
pub mod speculative;
pub mod kernel_adapter;
pub mod auto;
pub mod strategy;
pub mod pattern_find;
pub mod semantic_select;
pub mod multi_rewrite;
pub mod applicability;
pub mod stdlib;
