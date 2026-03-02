pub mod quickfix;
pub mod goaldirected;
pub mod rewrite;
pub mod witness;

use std::sync::Arc;
use crate::tactics::registry::TacticRegistry;
use crate::tactics::kernel_adapter::KernelTacticAdapter;
use crate::tactics::view::{ActionKind, ActionSafety};

pub fn register_std_tactics(registry: &mut TacticRegistry) {
    registry.register(Arc::new(quickfix::AddTouchTactic));
    registry.register(Arc::new(goaldirected::FocusGoalTactic));
    registry.register(Arc::new(rewrite::RewriteTactic));
    registry.register(Arc::new(witness::WitnessInsertTactic));
    // SC4: ExactTactic via KernelTacticAdapter
    registry.register(Arc::new(KernelTacticAdapter::new(
        "std.exact",
        "Close goal by exact hypothesis",
        crate::tactics::kernel_adapter::ExactTactic,
        ActionKind::QuickFix,
        ActionSafety::Safe,
    )));
}
