/**
 * SB3: Goals panel WebviewViewProvider.
 *
 * Registers an `edgelord.goalsPanel` webview view that displays proof goals
 * reactively via `$/edgelord/goalsUpdated` push notifications from the LSP
 * server.
 *
 * **INV S-PUSH**: panel always reflects the last pushed state; no polling.
 */

import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import type { GoalsUpdatedParams } from "./notifications";
import { renderGoalsHtml } from "./goalsPanelRenderer";

export const GOALS_VIEW_ID = "edgelord.goalsPanel";

// ─── Provider ────────────────────────────────────────────────────────────────

export class GoalsPanelProvider implements vscode.WebviewViewProvider {
  private view: vscode.WebviewView | undefined;
  private latestParams: GoalsUpdatedParams | null = null;
  private currentUri: string | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly client: LanguageClient
  ) {}

  // Called by VS Code when the panel becomes visible
  resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken
  ): void {
    this.view = webviewView;
    webviewView.webview.options = {
      enableScripts: true,
      // Restrict resource loading to the extension's media folder
      localResourceRoots: [this.context.extensionUri],
    };
    // Render immediately with whatever we have
    this.refresh();

    // Handle messages from the webview script (click-to-focus, explain, etc.)
    webviewView.webview.onDidReceiveMessage(
      (msg) => this.handleWebviewMessage(msg),
      undefined,
      this.context.subscriptions
    );
  }

  // ─── Public API ──────────────────────────────────────────────────────────

  /**
   * Called from the notification listener when the server pushes a goals update.
   */
  update(params: GoalsUpdatedParams): void {
    this.latestParams = params;
    this.currentUri = params.uri;
    this.refresh();
  }

  /**
   * Force a re-render (e.g. after theme change or panel reveal).
   */
  refresh(): void {
    if (!this.view) {
      return;
    }
    this.view.webview.html = renderGoalsHtml(
      this.latestParams,
      this.view.webview.cspSource
    );
  }

  // ─── Message handler ─────────────────────────────────────────────────────

  private handleWebviewMessage(msg: { command: string; anchorId?: string }): void {
    switch (msg.command) {
      case "focusGoal":
        if (msg.anchorId && this.currentUri) {
          this.focusGoal(this.currentUri, msg.anchorId).catch(console.error);
        }
        break;
    }
  }

  /**
   * Ask the server to resolve an anchor to a byte span, then jump the editor
   * cursor to the corresponding range.
   */
  private async focusGoal(uri: string, anchorId: string): Promise<void> {
    try {
      const result = await this.client.sendRequest<{ span: { start: vscode.Position; end: vscode.Position } | null } | { error: string }>(
        "workspace/executeCommand",
        {
          command: "edgelord/resolve-anchor",
          arguments: [{ uri, anchorId }],
        }
      );

      if (!result || "error" in result || !result.span) {
        return;
      }

      // Find any editor showing this URI
      const targetUri = vscode.Uri.parse(uri);
      const editor = vscode.window.visibleTextEditors.find(
        (e) => e.document.uri.toString() === targetUri.toString()
      );
      if (!editor) {
        // Open the file first
        const doc = await vscode.workspace.openTextDocument(targetUri);
        const openedEditor = await vscode.window.showTextDocument(doc);
        revealRange(openedEditor, result.span);
      } else {
        revealRange(editor, result.span);
      }
    } catch (e) {
      // Silently ignore — goal anchor may have become stale
    }
  }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function revealRange(
  editor: vscode.TextEditor,
  span: { start: { line: number; character: number }; end: { line: number; character: number } }
): void {
  const start = new vscode.Position(span.start.line, span.start.character);
  const end = new vscode.Position(span.end.line, span.end.character);
  const range = new vscode.Range(start, end);
  editor.selection = new vscode.Selection(start, start);
  editor.revealRange(range, vscode.TextEditorRevealType.InCenterIfOutsideViewport);
}
