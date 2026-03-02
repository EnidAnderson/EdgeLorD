import * as path from "path";
import {
  workspace,
  ExtensionContext,
  window,
  commands,
} from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
  Executable,
} from "vscode-languageclient/node";
import { GoalsPanelProvider, GOALS_VIEW_ID } from "./goalsPanel";
import type { GoalsUpdatedParams } from "./notifications";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext) {
  const config = workspace.getConfiguration("maclane");

  // Resolve server binary path
  let serverPath = config.get<string>("server.path", "");
  if (!serverPath) {
    const fs = require("fs");

    // Try to find the binary relative to the extension source (handles symlinks)
    const extReal = fs.realpathSync(context.extensionPath);
    const edgelordDir = path.resolve(extReal, "..");

    const candidates = [
      // Relative to EdgeLorD (extension parent)
      path.join(edgelordDir, "target", "release", "edgelord-lsp"),
      path.join(edgelordDir, "target", "debug", "edgelord-lsp"),
      // Workspace folder if open
      ...(workspace.workspaceFolders || []).flatMap((wf) => [
        path.join(wf.uri.fsPath, "EdgeLorD", "target", "release", "edgelord-lsp"),
        path.join(wf.uri.fsPath, "EdgeLorD", "target", "debug", "edgelord-lsp"),
      ]),
    ];

    const found = candidates.find((p) => fs.existsSync(p));
    if (found) {
      serverPath = found;
    } else {
      serverPath = "edgelord-lsp"; // fall back to PATH
    }

    console.log(`[Mac Lane] Resolved server path: ${serverPath}`);
  }

  const extraArgs = config.get<string[]>("server.extraArgs", []);

  const run: Executable = {
    command: serverPath,
    args: extraArgs,
    transport: TransportKind.stdio,
  };

  const serverOptions: ServerOptions = {
    run,
    debug: run, // same for debug mode
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "maclane" }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.maclane"),
    },
    outputChannelName: "Mac Lane (EdgeLorD)",
  };

  const lc = new LanguageClient(
    "maclane",
    "Mac Lane Language Server",
    serverOptions,
    clientOptions
  );
  client = lc;

  // Register commands
  context.subscriptions.push(
    commands.registerCommand("maclane.restartServer", async () => {
      await lc.restart();
      window.showInformationMessage("Mac Lane language server restarted.");
    })
  );

  context.subscriptions.push(
    commands.registerCommand("maclane.showGoals", async () => {
      const editor = window.activeTextEditor;
      if (!editor || editor.document.languageId !== "maclane") {
        window.showErrorMessage("No active .maclane file.");
        return;
      }
      try {
        const result = await lc.sendRequest("workspace/executeCommand", {
          command: "edgelord/goals",
          arguments: [{ textDocument: { uri: editor.document.uri.toString() } }],
        });
        if (result) {
          const channel = window.createOutputChannel("Mac Lane Goals");
          channel.clear();
          channel.appendLine(JSON.stringify(result, null, 2));
          channel.show();
        } else {
          window.showInformationMessage("No goals available.");
        }
      } catch (e: any) {
        window.showErrorMessage(`Goals request failed: ${e.message}`);
      }
    })
  );

  // SB3: Goals panel webview view
  const goalsPanel = new GoalsPanelProvider(context, lc);
  context.subscriptions.push(
    window.registerWebviewViewProvider(GOALS_VIEW_ID, goalsPanel, {
      webviewOptions: { retainContextWhenHidden: true },
    })
  );

  // SB1: Register notification listener before start() (vscode-languageclient v9 API).
  lc.onNotification("$/edgelord/goalsUpdated", (params: GoalsUpdatedParams) => {
    goalsPanel.update(params);
  });

  // SB0: Proof stepping commands
  context.subscriptions.push(
    commands.registerCommand("maclane.stepForward", async () => {
      const editor = window.activeTextEditor;
      if (!editor || editor.document.languageId !== "maclane") { return; }
      await lc.sendRequest("workspace/executeCommand", {
        command: "edgelord/step-forward",
        arguments: [editor.document.uri.toString()],
      });
    }),
    commands.registerCommand("maclane.stepBackward", async () => {
      const editor = window.activeTextEditor;
      if (!editor || editor.document.languageId !== "maclane") { return; }
      await lc.sendRequest("workspace/executeCommand", {
        command: "edgelord/step-backward",
        arguments: [editor.document.uri.toString()],
      });
    }),
    commands.registerCommand("maclane.gotoCursor", async () => {
      const editor = window.activeTextEditor;
      if (!editor || editor.document.languageId !== "maclane") { return; }
      const offset = editor.document.offsetAt(editor.selection.active);
      await lc.sendRequest("workspace/executeCommand", {
        command: "edgelord/goto-cursor",
        arguments: [{ uri: editor.document.uri.toString(), cursorOffset: offset }],
      });
    }),
    // SB2: Undo proof step
    commands.registerCommand("maclane.undoStep", async () => {
      const editor = window.activeTextEditor;
      if (!editor || editor.document.languageId !== "maclane") { return; }
      await lc.sendRequest("workspace/executeCommand", {
        command: "edgelord/undo-step",
        arguments: [editor.document.uri.toString()],
      });
    }),
    // Toggle goals panel
    commands.registerCommand("maclane.showGoalsPanel", async () => {
      await commands.executeCommand(`${GOALS_VIEW_ID}.focus`);
    })
  );

  // Start the client (and server)
  lc.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
