/**
 * SB3: Type definitions for EdgeLorD custom LSP notifications.
 *
 * These mirror the Rust structs in lsp.rs that implement the
 * `lsp_types::notification::Notification` trait.
 */

// ─── Goal model (mirrors GoalPanelItem in goals_panel.rs) ───────────────────

export type GoalStatus = "Unsolved" | "Blocked" | "SOLVED" | "Cycle" | "Error";

export interface GoalBlocker {
  goalId: string;
  description: string;
}

export type GoalChangeKind =
  | { kind: "Added" }
  | { kind: "Removed" }
  | { kind: "StatusChanged"; oldStatus: GoalStatus; newStatus: GoalStatus }
  | { kind: "TitleChanged"; oldTitle: string }
  | { kind: "BlockersChanged" }
  | { kind: "ContextSummaryChanged" };

export interface GoalDelta {
  changes: GoalChangeKind[];
}

export interface LspRange {
  start: { line: number; character: number };
  end: { line: number; character: number };
}

export interface GoalPanelItem {
  id: string;
  label: string;
  status: GoalStatus;
  range: LspRange | null;
  blockers: GoalBlocker[];
  delta: GoalDelta | null;
  summary: string;
}

// ─── GoalsUpdated notification ($/edgelord/goalsUpdated) ────────────────────

export interface GoalsUpdatedParams {
  uri: string;
  version: number;
  goals: GoalPanelItem[];
  stale: boolean;
  banner: string | null;
  checkedUpTo: number | null;
  totalGoals: number;
  unsolvedGoals: number;
  deltaSummary: string | null;
}

// ─── CheckedRegion notification ($/edgelord/checkedRegion) ──────────────────

export interface CheckedRegionParams {
  uri: string;
  checkedUpTo: number;
  formCount: number;
  totalForms: number;
}
