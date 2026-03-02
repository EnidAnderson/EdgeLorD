/**
 * SD0: Proof Tree Webview Provider
 *
 * Renders the hierarchical proof structure (grouped by owner: def/rule/top)
 * in a VS Code TreeView using `TreeDataProvider`.
 */
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

export const PROOF_TREE_VIEW_ID = "maclane.proofTree";

// ─── Data shapes (mirror Rust ProofStructure / GoalSummaryEntry) ────────────

export interface GoalSummaryEntry {
  anchor_id: string;
  name: string;
  status_tag: "unsolved" | "solved" | "blocked" | "error";
  type_summary: string;
  dependencies: string[];
}

export interface ProofStructure {
  groups: Record<string, GoalSummaryEntry[]>;
  total_goals: number;
  solved_goals: number;
}

// ─── Tree items ────────────────────────────────────────────────────────────

class GroupItem extends vscode.TreeItem {
  constructor(
    public readonly groupKey: string,
    public readonly goals: GoalSummaryEntry[],
  ) {
    const solved = goals.filter((g) => g.status_tag === "solved").length;
    super(
      groupKey,
      goals.length > 0
        ? vscode.TreeItemCollapsibleState.Expanded
        : vscode.TreeItemCollapsibleState.None,
    );
    this.description = `${solved}/${goals.length} solved`;
    this.contextValue = "proofGroup";
  }
}

class GoalItem extends vscode.TreeItem {
  constructor(public readonly entry: GoalSummaryEntry) {
    super(entry.name || entry.anchor_id, vscode.TreeItemCollapsibleState.None);
    this.description = entry.type_summary;
    this.tooltip = `${entry.name}: ${entry.type_summary}\nStatus: ${entry.status_tag}`;
    this.contextValue = `proofGoal-${entry.status_tag}`;

    const iconMap: Record<string, string> = {
      solved: "check",
      unsolved: "circle-outline",
      blocked: "warning",
      error: "error",
    };
    this.iconPath = new vscode.ThemeIcon(
      iconMap[entry.status_tag] ?? "circle-outline",
    );
  }
}

type TreeNode = GroupItem | GoalItem;

// ─── Provider ─────────────────────────────────────────────────────────────

export class ProofTreeProvider implements vscode.TreeDataProvider<TreeNode> {
  private _onDidChangeTreeData = new vscode.EventEmitter<
    TreeNode | undefined | void
  >();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private structure: ProofStructure | null = null;

  constructor(private readonly client: LanguageClient) {}

  refresh(structure: ProofStructure | null): void {
    this.structure = structure;
    this._onDidChangeTreeData.fire();
  }

  async fetchAndRefresh(uri: vscode.Uri): Promise<void> {
    try {
      const result = await this.client.sendRequest("workspace/executeCommand", {
        command: "edgelord/proof-structure",
        arguments: [{ uri: uri.toString() }],
      });
      if (result && typeof result === "object" && "groups" in result) {
        this.refresh(result as ProofStructure);
      } else {
        this.refresh(null);
      }
    } catch {
      this.refresh(null);
    }
  }

  getTreeItem(element: TreeNode): vscode.TreeItem {
    return element;
  }

  getChildren(element?: TreeNode): vscode.ProviderResult<TreeNode[]> {
    if (!this.structure) {
      return [];
    }
    if (!element) {
      // Root: one group item per owner key (BTreeMap → already sorted)
      return Object.entries(this.structure.groups).map(
        ([key, goals]) => new GroupItem(key, goals),
      );
    }
    if (element instanceof GroupItem) {
      return element.goals.map((g) => new GoalItem(g));
    }
    return [];
  }
}
