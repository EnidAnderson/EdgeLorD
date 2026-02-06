pub mod lsp;
pub mod document;
pub mod proof_session;
pub mod span_conversion; // Added this line
pub mod goals_panel;
pub mod edgelord_pretty_ctx; // Added this line
pub mod explain;
pub mod tactics;
pub mod diff;
pub mod proposal;
pub mod loogle;

pub use lsp::Backend;
