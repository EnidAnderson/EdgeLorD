/**
 * SB3: Goals panel HTML renderer.
 *
 * Converts a `GoalsUpdatedParams` payload into the HTML content displayed
 * inside the EdgeLorD Goals webview panel.
 */

import type { GoalsUpdatedParams, GoalPanelItem, GoalStatus, GoalChangeKind } from "./notifications";

// ─── Public API ──────────────────────────────────────────────────────────────

/**
 * Render the full webview HTML document for the given parameters.
 * Returns a complete `<!DOCTYPE html>` string safe to pass to
 * `webview.html`.
 */
export function renderGoalsHtml(
  params: GoalsUpdatedParams | null,
  webviewCspSource: string
): string {
  const body = params ? renderBody(params) : renderEmpty();
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy"
        content="default-src 'none'; style-src ${webviewCspSource} 'unsafe-inline'; script-src ${webviewCspSource} 'unsafe-inline';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>EdgeLorD Goals</title>
  <style>${inlineCss()}</style>
</head>
<body>
  ${body}
  <script>${inlineScript()}</script>
</body>
</html>`;
}

// ─── Body rendering ──────────────────────────────────────────────────────────

function renderEmpty(): string {
  return `<div class="empty-state">
    <p>No active Mac Lane file. Open a <code>.maclane</code> file to see goals.</p>
  </div>`;
}

function renderBody(params: GoalsUpdatedParams): string {
  const staleBar = params.stale
    ? `<div class="stale-bar">⏳ Checking…</div>`
    : "";

  const banner = params.banner
    ? `<div class="banner">${escapeHtml(params.banner)}</div>`
    : "";

  const goals =
    params.goals.length === 0
      ? `<div class="no-goals">✅ No remaining goals.</div>`
      : params.goals.map(renderGoal).join("\n");

  const footer = renderFooter(params);

  return `<div class="goals-panel">
  ${staleBar}
  ${banner}
  <div class="goals-list">${goals}</div>
  ${footer}
</div>`;
}

function renderGoal(goal: GoalPanelItem): string {
  const statusClass = statusCssClass(goal.status);
  const icon = statusIcon(goal.status);
  const deltaHtml = goal.delta ? renderDelta(goal.delta) : "";

  const blockersHtml =
    goal.blockers.length > 0
      ? `<div class="goal-blockers">
           Blocked by: ${goal.blockers
             .map((b) => `<span class="blocker">${escapeHtml(b.description)}</span>`)
             .join(", ")}
         </div>`
      : "";

  return `<div class="goal ${statusClass}"
              data-anchor="${escapeAttr(goal.id)}"
              data-range='${JSON.stringify(goal.range ?? null)}'>
  <div class="goal-header">
    <span class="goal-status-icon">${icon}</span>
    <span class="goal-label">${escapeHtml(goal.label)}</span>
    ${deltaHtml}
    <button class="focus-btn" title="Jump to goal" data-anchor="${escapeAttr(goal.id)}">⤴</button>
  </div>
  <div class="goal-summary"><code>${escapeHtml(goal.summary)}</code></div>
  ${blockersHtml}
</div>`;
}

function renderDelta(delta: { changes: GoalChangeKind[] }): string {
  const tags = delta.changes
    .map((c) => {
      switch (c.kind) {
        case "Added":        return `<span class="delta delta-added">+new</span>`;
        case "Removed":      return `<span class="delta delta-removed">−removed</span>`;
        case "StatusChanged":
          return `<span class="delta delta-status">${escapeHtml(c.oldStatus)} → ${escapeHtml(c.newStatus)}</span>`;
        default:             return "";
      }
    })
    .filter(Boolean)
    .join(" ");
  return tags ? `<span class="goal-delta">${tags}</span>` : "";
}

function renderFooter(params: GoalsUpdatedParams): string {
  const solved = params.totalGoals - params.unsolvedGoals;
  const delta = params.deltaSummary ? ` · ${escapeHtml(params.deltaSummary)}` : "";
  const checked =
    params.checkedUpTo != null
      ? ` · Checked: ${params.checkedUpTo} bytes`
      : "";
  return `<div class="goals-footer">
    ${solved}/${params.totalGoals} solved${delta}${checked}
  </div>`;
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function statusCssClass(s: GoalStatus): string {
  switch (s) {
    case "Unsolved":  return "goal-unsolved";
    case "Blocked":   return "goal-blocked";
    case "SOLVED":    return "goal-solved";
    case "Cycle":     return "goal-cycle";
    case "Error":     return "goal-error";
    default:          return "goal-unknown";
  }
}

function statusIcon(s: GoalStatus): string {
  switch (s) {
    case "Unsolved":  return "⬜";
    case "Blocked":   return "🟧";
    case "SOLVED":    return "✅";
    case "Cycle":     return "🔄";
    case "Error":     return "❌";
    default:          return "❓";
  }
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeAttr(s: string): string {
  return s.replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

// ─── Inline CSS ──────────────────────────────────────────────────────────────

function inlineCss(): string {
  return `
:root {
  --goal-unsolved: var(--vscode-editorWarning-foreground, #cca700);
  --goal-solved:   var(--vscode-testing-iconPassed, #3fb950);
  --goal-blocked:  var(--vscode-editorInfo-foreground, #75beff);
  --goal-error:    var(--vscode-editorError-foreground, #f14c4c);
  --bg:            var(--vscode-editor-background, #1e1e1e);
  --fg:            var(--vscode-editor-foreground, #d4d4d4);
  --border:        var(--vscode-panel-border, #3c3c3c);
  --badge-bg:      var(--vscode-badge-background, #4d4d4d);
  --hover:         var(--vscode-list-hoverBackground, #2a2d2e);
  font-family:     var(--vscode-editor-font-family, monospace);
  font-size:       var(--vscode-editor-font-size, 13px);
}
body { margin: 0; padding: 0; background: var(--bg); color: var(--fg); }
.goals-panel { display: flex; flex-direction: column; height: 100vh; overflow: hidden; }
.stale-bar { background: var(--badge-bg); padding: 4px 8px; font-size: 11px; color: var(--fg); }
.banner { padding: 4px 8px; background: var(--badge-bg); border-bottom: 1px solid var(--border); font-size: 12px; }
.goals-list { flex: 1; overflow-y: auto; padding: 4px 0; }
.goal { border-bottom: 1px solid var(--border); padding: 6px 10px; cursor: pointer; }
.goal:hover { background: var(--hover); }
.goal-header { display: flex; align-items: center; gap: 6px; }
.goal-label { flex: 1; font-weight: bold; }
.goal-summary code { display: block; margin-top: 4px; font-size: 12px; opacity: 0.85; white-space: pre-wrap; }
.goal-blockers { margin-top: 4px; font-size: 11px; opacity: 0.75; }
.blocker { background: var(--badge-bg); border-radius: 3px; padding: 0 4px; margin: 0 2px; }
.goal-delta { font-size: 11px; }
.delta { border-radius: 3px; padding: 1px 4px; margin: 0 2px; }
.delta-added   { background: #1e4620; color: var(--goal-solved); }
.delta-removed { background: #4b1010; color: var(--goal-error); }
.delta-status  { background: var(--badge-bg); }
.goal-unsolved .goal-label { color: var(--goal-unsolved); }
.goal-solved   .goal-label { color: var(--goal-solved); }
.goal-blocked  .goal-label { color: var(--goal-blocked); }
.goal-error    .goal-label { color: var(--goal-error); }
.goal-solved   { opacity: 0.6; }
.goal-solved   .goal-summary code { text-decoration: line-through; }
.focus-btn { background: none; border: none; color: var(--fg); cursor: pointer; opacity: 0.5; font-size: 14px; padding: 0 4px; }
.focus-btn:hover { opacity: 1; }
.goals-footer { border-top: 1px solid var(--border); padding: 4px 10px; font-size: 11px; opacity: 0.7; }
.no-goals { padding: 20px 10px; text-align: center; opacity: 0.7; }
.empty-state { padding: 20px 10px; text-align: center; opacity: 0.6; }
`;
}

// ─── Inline JS (message passing to extension host) ───────────────────────────

function inlineScript(): string {
  return `
const vscode = acquireVsCodeApi();

// Click on goal header → focus
document.addEventListener('click', (e) => {
  const btn = e.target.closest('.focus-btn');
  if (btn) {
    const anchor = btn.dataset.anchor;
    vscode.postMessage({ command: 'focusGoal', anchorId: anchor });
    e.stopPropagation();
    return;
  }
  const goal = e.target.closest('.goal');
  if (goal) {
    const anchor = goal.dataset.anchor;
    vscode.postMessage({ command: 'focusGoal', anchorId: anchor });
  }
});

// Calm Mode toggle (future)
`;
}
