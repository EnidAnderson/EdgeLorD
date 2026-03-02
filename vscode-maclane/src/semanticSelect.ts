/**
 * SE2: Avy-jump Semantic Selection
 *
 * Shows a label overlay for all pattern sites then prompts the user to
 * pick a label character. The selected occurrence is then passed to a
 * callback (e.g. to seed a multi-rewrite or code action).
 */
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import { PatternOverlayManager } from "./patternOverlay";

export interface SemanticSelectResult {
  label: string;
  patternName: string;
  occurrence: unknown;
}

export class SemanticSelectController {
  private overlay: PatternOverlayManager;

  constructor(private readonly lc: LanguageClient) {
    this.overlay = new PatternOverlayManager(lc);
  }

  dispose(): void {
    this.overlay.dispose();
  }

  /**
   * Show overlay for `patternName`, prompt for a label, return the selected
   * occurrence (or `undefined` if cancelled).
   */
  async pick(
    editor: vscode.TextEditor,
    patternName: string,
  ): Promise<SemanticSelectResult | undefined> {
    const labels = await this.overlay.show(editor, patternName);
    if (labels.length === 0) {
      vscode.window.showInformationMessage(
        `No occurrences of '${patternName}' found.`,
      );
      return undefined;
    }

    const chosen = await vscode.window.showQuickPick(
      labels.map((l) => ({ label: l, description: `site ${l}` })),
      { placeHolder: `Select pattern site for '${patternName}'` },
    );
    this.overlay.clear();

    if (!chosen) return undefined;

    // Resolve the occurrence by label via the server
    let occurrence: unknown;
    try {
      occurrence = await this.lc.sendRequest("workspace/executeCommand", {
        command: "edgelord/select-pattern-site",
        arguments: [
          {
            uri: editor.document.uri.toString(),
            pattern: patternName,
            label: chosen.label,
          },
        ],
      });
    } catch {
      return undefined;
    }

    if (!occurrence || (occurrence as any).error) {
      vscode.window.showErrorMessage(
        `Could not resolve label '${chosen.label}'.`,
      );
      return undefined;
    }

    return { label: chosen.label, patternName, occurrence };
  }
}

/** Standalone command: pick a pattern and avy-jump to it. */
export async function avySelectCommand(lc: LanguageClient): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "maclane") {
    vscode.window.showErrorMessage("No active .maclane file.");
    return;
  }

  const pattern = await vscode.window.showInputBox({
    prompt: "Pattern name for semantic selection",
    placeHolder: "e.g. suspension, tate-twist",
  });
  if (!pattern) return;

  const controller = new SemanticSelectController(lc);
  const result = await controller.pick(editor, pattern);
  controller.dispose();

  if (result) {
    vscode.window.showInformationMessage(
      `Selected site '${result.label}' for pattern '${result.patternName}'.`,
    );
  }
}
