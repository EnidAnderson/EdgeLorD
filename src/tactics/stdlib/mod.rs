pub mod quickfix;
pub mod goaldirected;
pub mod rewrite;

use std::sync::Arc;
use crate::tactics::registry::TacticRegistry;

pub fn register_std_tactics(registry: &mut TacticRegistry) {
    registry.register(Arc::new(quickfix::AddTouchTactic));
    registry.register(Arc::new(goaldirected::FocusGoalTactic));
}
