use serde::{Serialize, Deserialize};
use tower_lsp::lsp_types::Range;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalsPanelResponse {
    pub uri: String,
    pub goals: Vec<GoalPanelItem>,
    pub version: i32,
    pub stale: bool,
    pub banner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalPanelItem {
    pub id: String,          // Stable Anchor ID
    pub label: String,       // "?name : Type"
    pub status: GoalStatus,
    pub range: Range,        // Current source range
    pub blockers: Vec<BlockerInfo>,
    pub delta: Option<GoalDelta>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "camelCase")]
pub enum GoalStatus {
    Unsolved,
    Blocked,
    SOLVED, 
    Cycle,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockerInfo {
    pub id: String, // Constraint anchor ID or description
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GoalChangeKind {
    Added,
    Removed,
    StatusChanged { 
        #[serde(rename = "from")]
        old_status: GoalStatus, 
        #[serde(rename = "to")]
        new_status: GoalStatus 
    },
    TitleChanged,
    BlockersChanged { 
        added: Vec<String>, 
        removed: Vec<String> 
    },
    ContextSummaryChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalDelta {
    pub changes: Vec<GoalChangeKind>,
}
