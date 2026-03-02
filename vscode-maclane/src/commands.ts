/**
 * SD1 / SD2 / SD3 / SE0 / SE2 / SE3 / SE4: Command implementations
 *
 * All commands follow the same pattern:
 *   1. Require an active .maclane editor
 *   2. Send a workspace/executeCommand to the LSP
 *   3. Navigate / display result as appropriate
 */
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

// ─── Helpers ──────────────────────────────────────────────────────────────

function requireMaclaneEditor(): vscode.TextEditor | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "maclane") {
    vscode.window.showErrorMessage("No active .maclane file.");
    return undefined;
  }
  return editor;
}

async function executeCommand(
  lc: LanguageClient,
  command: string,
  args: unknown[],
): Promise<unknown> {
  return lc.sendRequest("workspace/executeCommand", {
    command,
    arguments: args,
  });
}

function lspRangeToVsRange(r: {
  start: { line: number; character: number };
  end: { line: number; character: number };
}): vscode.Range {
  return new vscode.Range(
    new vscode.Position(r.start.line, r.start.character),
    new vscode.Position(r.end.line, r.end.character),
  );
}

async function navigateToRange(
  editor: vscode.TextEditor,
  range: vscode.Range,
): Promise<void> {
  editor.selection = new vscode.Selection(range.start, range.end);
  editor.revealRange(range, vscode.TextEditorRevealType.InCenterIfOutsideViewport);
}

// ─── SD1: Goal navigation ─────────────────────────────────────────────────

export async function nextGoal(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const offset = editor.document.offsetAt(editor.selection.active);
  const result: any = await executeCommand(lc, "edgelord/next-goal", [
    { uri: editor.document.uri.toString(), offset },
  ]);
  if (!result || result.error) {
    vscode.window.showInformationMessage(
      result?.error ?? "No next goal found.",
    );
    return;
  }
  await navigateToRange(editor, lspRangeToVsRange(result.span));
}

export async function prevGoal(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const offset = editor.document.offsetAt(editor.selection.active);
  const result: any = await executeCommand(lc, "edgelord/prev-goal", [
    { uri: editor.document.uri.toString(), offset },
  ]);
  if (!result || result.error) {
    vscode.window.showInformationMessage(
      result?.error ?? "No previous goal found.",
    );
    return;
  }
  await navigateToRange(editor, lspRangeToVsRange(result.span));
}

export async function nextBlocker(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const offset = editor.document.offsetAt(editor.selection.active);
  const result: any = await executeCommand(lc, "edgelord/next-blocker", [
    { uri: editor.document.uri.toString(), offset },
  ]);
  if (!result || result.error) {
    vscode.window.showInformationMessage(
      result?.error ?? "No blocker found at cursor.",
    );
    return;
  }
  await navigateToRange(editor, lspRangeToVsRange(result.span));
}

// ─── SD2: Auto-tactic ────────────────────────────────────────────────────

export async function autoTactic(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const result: any = await executeCommand(lc, "edgelord/auto", [
    { uri: editor.document.uri.toString() },
  ]);
  if (!result) {
    vscode.window.showInformationMessage("Auto-tactic: no result.");
    return;
  }
  const tag = result.Solved
    ? "Solved"
    : result.Partial
      ? "Partial"
      : result.Stuck
        ? "Stuck"
        : "Exhausted";
  vscode.window.showInformationMessage(`Auto-tactic: ${tag}`);
}

// ─── SD3: Apply strategy ─────────────────────────────────────────────────

export async function applyStrategy(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const strategies = ["motivic-standard", "differential-standard"];
  const choice = await vscode.window.showQuickPick(strategies, {
    placeHolder: "Select proof strategy",
  });
  if (!choice) return;
  const result: any = await executeCommand(lc, "edgelord/apply-strategy", [
    { uri: editor.document.uri.toString(), strategy: choice },
  ]);
  if (!result) {
    vscode.window.showInformationMessage("Strategy: no result.");
    return;
  }
  const summary = `Strategy applied: ${result.applied_phases?.length ?? 0} phase(s), solved=${result.solved}`;
  vscode.window.showInformationMessage(summary);
}

// ─── SE0: Find pattern occurrences ───────────────────────────────────────

export async function findPattern(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const pattern = await vscode.window.showInputBox({
    prompt: "Enter pattern name to find",
    placeHolder: "e.g. suspension, tate-twist",
  });
  if (!pattern) return;
  const result: any = await executeCommand(lc, "edgelord/find-pattern", [
    { uri: editor.document.uri.toString(), pattern },
  ]);
  const occurrences: any[] = Array.isArray(result) ? result : [];
  if (occurrences.length === 0) {
    vscode.window.showInformationMessage(`No occurrences of '${pattern}' found.`);
    return;
  }
  vscode.window.showInformationMessage(
    `Found ${occurrences.length} occurrence(s) of '${pattern}'.`,
  );
}

// ─── SE3: Multi-site rewrite ─────────────────────────────────────────────

export async function multiRewrite(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const rule = await vscode.window.showInputBox({
    prompt: "Enter rule name for multi-site rewrite",
  });
  if (!rule) return;
  const result: any = await executeCommand(lc, "edgelord/multi-rewrite", [
    { uri: editor.document.uri.toString(), rule },
  ]);
  if (!result) {
    vscode.window.showInformationMessage("Multi-rewrite: no result.");
    return;
  }
  if (result.conflicts && result.conflicts.length > 0) {
    vscode.window.showWarningMessage(
      `Multi-rewrite: ${result.conflicts.length} conflict(s) detected. No edits applied.`,
    );
    return;
  }
  // Apply workspace edit
  const wsEdit = new vscode.WorkspaceEdit();
  if (result.edits && Array.isArray(result.edits)) {
    for (const edit of result.edits) {
      if (edit.range && edit.new_text !== undefined) {
        wsEdit.replace(
          editor.document.uri,
          lspRangeToVsRange(edit.range),
          edit.new_text,
        );
      }
    }
  }
  if (wsEdit.size > 0) {
    await vscode.workspace.applyEdit(wsEdit);
    vscode.window.showInformationMessage(
      `Multi-rewrite: applied ${result.edits?.length ?? 0} site(s).`,
    );
  } else {
    vscode.window.showInformationMessage("Multi-rewrite: no applicable sites.");
  }
}

// ─── SE4: Tactic applicability ────────────────────────────────────────────

export async function tacticApplicability(lc: LanguageClient): Promise<void> {
  const editor = requireMaclaneEditor();
  if (!editor) return;
  const rule = await vscode.window.showInputBox({
    prompt: "Enter rule name to check applicability",
  });
  if (!rule) return;
  const result: any = await executeCommand(lc, "edgelord/tactic-applicability", [
    { uri: editor.document.uri.toString(), rule },
  ]);
  if (!result) {
    vscode.window.showInformationMessage("Applicability: no result.");
    return;
  }
  const wouldSolve = result.would_solve?.length ?? 0;
  const totalSites = result.total_sites ?? 0;
  vscode.window.showInformationMessage(
    `'${rule}': ${totalSites} site(s), would solve ${wouldSolve} goal(s).`,
  );
}
