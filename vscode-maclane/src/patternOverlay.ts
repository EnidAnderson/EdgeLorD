/**
 * SE1: Pattern Highlight Overlay
 *
 * Fetches pattern occurrences via `edgelord/find-pattern` and renders
 * label decorations (a, b, c …) over each site in the active editor.
 *
 * **INV D-***: Labels are deterministic — based on server-side index order.
 */
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

// ─── Decoration type ─────────────────────────────────────────────────────

const OVERLAY_DECORATION = vscode.window.createTextEditorDecorationType({
  backgroundColor: new vscode.ThemeColor("editor.findMatchHighlightBackground"),
  borderWidth: "1px",
  borderStyle: "solid",
  borderColor: new vscode.ThemeColor("editorBracketHighlight.foreground1"),
  borderRadius: "2px",
});

// ─── Overlay manager ─────────────────────────────────────────────────────

export class PatternOverlayManager {
  private activePattern: string | null = null;
  private activeEditor: vscode.TextEditor | null = null;

  constructor(private readonly lc: LanguageClient) {}

  dispose(): void {
    this.clear();
  }

  clear(): void {
    if (this.activeEditor) {
      this.activeEditor.setDecorations(OVERLAY_DECORATION, []);
    }
    this.activePattern = null;
    this.activeEditor = null;
  }

  async show(editor: vscode.TextEditor, patternName: string): Promise<string[]> {
    // Fetch occurrences from server
    let result: unknown;
    try {
      result = await this.lc.sendRequest("workspace/executeCommand", {
        command: "edgelord/find-pattern",
        arguments: [{ uri: editor.document.uri.toString(), pattern: patternName }],
      });
    } catch {
      return [];
    }

    const occurrences: any[] = Array.isArray(result) ? result : [];
    if (occurrences.length === 0) {
      this.clear();
      return [];
    }

    // Build decorations + collect labels
    const decorations: vscode.DecorationOptions[] = [];
    const labels: string[] = [];

    for (let i = 0; i < occurrences.length; i++) {
      const occ = occurrences[i];
      const label = generateLabel(i);
      labels.push(label);

      if (occ.span) {
        const range = new vscode.Range(
          new vscode.Position(occ.span.start.line, occ.span.start.character),
          new vscode.Position(occ.span.end.line, occ.span.end.character),
        );
        decorations.push({
          range,
          hoverMessage: `Pattern site ${label}: ${occ.rule_name ?? patternName}`,
          renderOptions: {
            after: {
              contentText: ` [${label}]`,
              color: new vscode.ThemeColor("editorBracketHighlight.foreground1"),
              fontStyle: "italic",
            },
          },
        });
      }
    }

    this.activeEditor = editor;
    this.activePattern = patternName;
    editor.setDecorations(OVERLAY_DECORATION, decorations);
    return labels;
  }
}

/** Mirror of server-side `generate_label`: 0→"a", 25→"z", 26→"aa", … */
function generateLabel(index: number): string {
  const alphabet = "abcdefghijklmnopqrstuvwxyz";
  if (index < 26) {
    return alphabet[index];
  }
  const outer = Math.floor(index / 26) - 1;
  const inner = index % 26;
  return alphabet[outer] + alphabet[inner];
}
