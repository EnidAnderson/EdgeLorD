use std::{collections::{BTreeMap, VecDeque}, sync::Arc};
// Removed use std::default::Default; (was not used anyway)

use tokio::sync::RwLock;
use tokio::time::Instant;
use tower_lsp::{
    lsp_types::{Diagnostic, MessageType, TextDocumentContentChangeEvent, Url},
    Client, // Added Client import
};

use crate::document::{apply_content_changes, ParsedDocument, Goal};
use crate::lsp::{Config, workspace_error_report, document_diagnostics_from_report};
use new_surface_syntax::comrade_workspace::WorkspaceReport;
use new_surface_syntax::ComradeWorkspace;
// Removed use crate::client_sink::ClientSink;
use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;
use new_surface_syntax::diagnostics::DiagnosticContext;

#[derive(Clone)]
pub struct ProofSnapshot {
    pub version: i32,
    pub timestamp: Instant,
    pub proof_state: new_surface_syntax::proof_state::ProofState,
    pub goals_index: GoalsPanelIndex,
}

pub struct ProofDocument {
    pub version: i32,
    pub parsed: ParsedDocument,
    pub last_analyzed: Instant,
    pub workspace_report: WorkspaceReport,
    pub goals_index: Option<GoalsPanelIndex>, // NEW: Stable anchor index
    pub history: VecDeque<ProofSnapshot>, // NEW: Semantic snapshots
}

pub struct ProofSessionOpenResult {
    pub report: WorkspaceReport,
    pub diagnostics: Vec<Diagnostic>,
    pub goals: Vec<Goal>,
}

pub struct ProofSessionUpdateResult {
    pub report: WorkspaceReport,
    pub diagnostics: Vec<Diagnostic>,
    pub goals: Vec<Goal>,
}

pub struct ProofSession {
    client: Client, // Changed from Arc<dyn ClientSink> to Client
    config: Arc<RwLock<Config>>,
    workspace: ComradeWorkspace,
    documents: BTreeMap<Url, ProofDocument>,
    loogle_indexer: Arc<crate::loogle::WorkspaceIndexer>,
}

impl ProofSession {
    pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type
        let loogle_indexer = Arc::new(
            crate::loogle::WorkspaceIndexer::new()
                .expect("Failed to initialize Loogle indexer")
        );
        
        Self {
            client,
            config,
            workspace: ComradeWorkspace::new(),
            documents: BTreeMap::new(),
            loogle_indexer,
        }
    }

    pub async fn open(&mut self, uri: Url, version: i32, initial_text: String) -> ProofSessionOpenResult {
        let key = uri.to_string();
        let parsed = ParsedDocument::parse(initial_text.clone());

        let report = match self.workspace.did_open(&key, &initial_text) {
            Ok(report) => report,
            Err(err) => workspace_error_report(&err),
        };

        let diagnostics = document_diagnostics_from_report(&uri, &report, &parsed);
        // Compute stable goals index if ProofState is available
        let goals_index = if let Some(ref ps) = report.proof_state {
            let diag_ctx = DiagnosticContext::new(key.clone(), &key);
            Some(GoalsPanelIndex::new(ps, &diag_ctx))
        } else {
            None
        };
        
        // Use stable goals if available, otherwise fallback to parsed (syntactic) goals
        let goals = if let (Some(index), Some(ps)) = (&goals_index, &report.proof_state) {
            compute_ui_goals(&uri, &parsed, ps, index)
        } else {
            parsed.goals.clone()
        };

        let mut history = VecDeque::new();
        if let (Some(ps), Some(index)) = (&report.proof_state, &goals_index) {
            history.push_back(ProofSnapshot {
                version,
                timestamp: Instant::now(),
                proof_state: ps.clone(),
                goals_index: index.clone(),
            });
        }

        // Reindex workspace for Loogle if bundle is available
        if let Some(ref bundle) = report.bundle {
            if let Err(e) = self.loogle_indexer.reindex(bundle) {
                self.client.log_message(
                    MessageType::WARNING,
                    format!("Loogle indexing failed: {}", e)
                ).await;
            }
        }

        self.documents.insert(uri, ProofDocument {
            version,
            parsed,
            last_analyzed: Instant::now(),
            workspace_report: report.clone(),
            goals_index,
            history,
        });

        ProofSessionOpenResult {
            report,
            diagnostics,
            goals,
        }
    }

    pub async fn update(&mut self, uri: Url, version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> ProofSessionUpdateResult {
        let Some(current_proof_doc) = self.documents.get(&uri) else {
            // Updated call to log_message to use direct Client method
            self.client.log_message(MessageType::ERROR, format!("ProofSession: Attempted to update non-existent document: {}", uri).to_string()).await;
            return ProofSessionUpdateResult {
                report: WorkspaceReport { diagnostics: Vec::new(), fingerprint: None, revision: 0, bundle: None, proof_state: None },
                diagnostics: Vec::new(),
                goals: Vec::new(),
            };
        };

        let updated_text = apply_content_changes(&current_proof_doc.parsed.text, &changes);
        let parsed = ParsedDocument::parse(updated_text.clone());
        let key = uri.to_string();

        let content_changes_for_workspace = changes.into_iter().map(|change| {
            if let Some(range) = change.range {
                let start = crate::document::position_to_offset(&current_proof_doc.parsed.text, range.start);
                let end = crate::document::position_to_offset(&current_proof_doc.parsed.text, range.end);
                new_surface_syntax::ContentChange {
                    range: Some((start, end)),
                    text: change.text,
                }
            } else {
                new_surface_syntax::ContentChange {
                    range: None,
                    text: change.text,
                }
            }
        }).collect::<Vec<_>>();


        let report = match self.workspace.did_change(&key, &content_changes_for_workspace) {
            Ok(report) => report,
            Err(err) => workspace_error_report(&err),
        };

        let diagnostics = document_diagnostics_from_report(&uri, &report, &parsed);
        // Compute stable goals index if ProofState is available
        let goals_index = if let Some(ref ps) = report.proof_state {
            let diag_ctx = DiagnosticContext::new(key.clone(), &key);
            Some(GoalsPanelIndex::new(ps, &diag_ctx))
        } else {
            None
        };
        
        // Use stable goals if available, otherwise fallback to parsed (syntactic) goals
        let goals = if let (Some(index), Some(ps)) = (&goals_index, &report.proof_state) {
            compute_ui_goals(&uri, &parsed, ps, index)
        } else {
            parsed.goals.clone()
        };

        let mut history = current_proof_doc.history.clone();
        if let (Some(ps), Some(index)) = (&report.proof_state, &goals_index) {
            history.push_back(ProofSnapshot {
                version,
                timestamp: Instant::now(),
                proof_state: ps.clone(),
                goals_index: index.clone(),
            });
            if history.len() > 10 {
                history.pop_front();
            }
        }

        // Reindex workspace for Loogle if bundle is available
        if let Some(ref bundle) = report.bundle {
            if let Err(e) = self.loogle_indexer.reindex(bundle) {
                self.client.log_message(
                    MessageType::WARNING,
                    format!("Loogle indexing failed: {}", e)
                ).await;
            }
        }

        self.documents.insert(uri, ProofDocument {
            version,
            parsed,
            last_analyzed: Instant::now(),
            workspace_report: report.clone(),
            goals_index,
            history,
        });

        ProofSessionUpdateResult {
            report,
            diagnostics,
            goals,
        }
    }

    pub fn get_document(&self, uri: &Url) -> Option<&ProofDocument> {
        self.documents.get(uri)
    }

    pub fn get_goals(&self, uri: &Url) -> Vec<Goal> {
        self.documents
            .get(uri)
            .map(|doc| doc.parsed.goals.clone())
            .unwrap_or_default()
    }

    pub fn loogle_index(&self) -> &crate::loogle::WorkspaceIndexer {
        &self.loogle_indexer
    }

    pub async fn apply_command(&mut self, uri: Url, _command: String) -> ProofSessionUpdateResult {
        let Some(current_proof_doc) = self.documents.get(&uri) else {
            // Updated call to log_message to use direct Client method
            self.client.log_message(MessageType::ERROR, format!("ProofSession: Attempted to apply command to non-existent document: {}", uri).to_string()).await;
            return ProofSessionUpdateResult {
                report: WorkspaceReport { diagnostics: Vec::new(), fingerprint: None, revision: 0, bundle: None, proof_state: None },
                diagnostics: Vec::new(),
                goals: Vec::new(),
            };
        };

        let key = uri.to_string();
        let report = match self.workspace.did_change(&key, &[]) {
            Ok(report) => report,
            Err(err) => workspace_error_report(&err),
        };

        let diagnostics = document_diagnostics_from_report(&uri, &report, &current_proof_doc.parsed);
        let goals = current_proof_doc.parsed.goals.clone();

        ProofSessionUpdateResult {
            report,
            diagnostics,
            goals,
        }
    }

    pub fn get_document_text(&self, uri: &Url) -> Option<String> {
        self.documents.get(uri).map(|doc| doc.parsed.text.clone())
    }

    pub fn get_document_version(&self, uri: &Url) -> Option<i32> {
        self.documents.get(uri).map(|doc| doc.version)
    }

    pub fn get_last_analyzed_time(&self, uri: &Url) -> Option<Instant> {
        self.documents.get(uri).map(|doc| doc.last_analyzed)
    }

    pub fn get_parsed_document(&self, uri: &Url) -> Option<&ParsedDocument> {
        self.documents.get(uri).map(|doc| &doc.parsed)
    }

    pub fn get_proof_state(&self, uri: &Url) -> Option<&new_surface_syntax::proof_state::ProofState> {
        self.documents.get(uri).and_then(|doc| doc.workspace_report.proof_state.as_ref())
    }

    pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        self.documents
            .get(uri)
            .map(|doc| {
                let temp_report = doc.workspace_report.clone();
                let parsed_doc = &doc.parsed;
                document_diagnostics_from_report(uri, &temp_report, parsed_doc)

            })
            .unwrap_or_default()
    }

    pub fn resolve_goal_anchor(&self, uri: &Url, anchor_id: &str) -> Option<(new_surface_syntax::proof_state::MorMetaId, Option<new_surface_syntax::diagnostics::ByteSpan>)> {
        self.documents
            .get(uri)
            .and_then(|doc| doc.goals_index.as_ref())
            .and_then(|index| index.resolve_anchor(anchor_id))
    }

    pub fn close(&mut self, uri: &Url) {
        if let Some(_) = self.documents.remove(uri) {
            self.workspace.did_close(&uri.to_string());
        }
    }

    pub fn compute_goals_panel(&self, uri: &Url) -> Option<crate::goals_panel::GoalsPanelResponse> {
    let doc = self.documents.get(uri)?;
    let index = doc.goals_index.as_ref()?;
    let ps = doc.workspace_report.proof_state.as_ref()?;

    let id_to_goal: std::collections::HashMap<_, _> = ps.goals.iter().map(|g| (g.id, g)).collect();

    // Compute semantic deltas if history is available
    let deltas = if doc.history.len() > 1 {
        let current = doc.history.back().unwrap();
        let prev = &doc.history[doc.history.len() - 2];
        crate::diff::engine::compute_diff(&prev.proof_state, &prev.goals_index, &current.proof_state, &current.goals_index)
    } else {
        std::collections::BTreeMap::new()
    };

    let mut items = Vec::new();
    
    // Calm Mode detection
    let mut banner = None;
    let calm_mode = ps.goals.len() > 20; // Example threshold
    if calm_mode {
        banner = Some(format!("Calm Mode: Showing top frontier goals only ({} total)", ps.goals.len()));
    }

    // Filter goals if Calm Mode is active or by default?
    // Let's implement FrontierRelevant filtering.
    let mut sorted_goals: Vec<_> = ps.goals.iter().collect();
    if calm_mode {
        // FrontierStrict: zero unsolved dependencies.
        let is_frontier = |g: &&new_surface_syntax::proof_state::GoalState| {
            match &g.status {
                new_surface_syntax::proof_state::GoalStatus::Unsolved => true,
                new_surface_syntax::proof_state::GoalStatus::Blocked { depends_on } => depends_on.is_empty(),
                _ => false,
            }
        };
        
        // FrontierRelevant(1): Frontier + their blockers
        let mut relevant_ids = std::collections::BTreeSet::new();
        for g in sorted_goals.iter().filter(|g| is_frontier(g)) {
            relevant_ids.insert(g.id);
        }
        
        // Add one level of blockers for relevant goals
        let mut blockers = std::collections::BTreeSet::new();
        for id in &relevant_ids {
            if let Some(g) = id_to_goal.get(id) {
                if let new_surface_syntax::proof_state::GoalStatus::Blocked { depends_on } = &g.status {
                    for dep in depends_on {
                        blockers.insert(*dep);
                    }
                }
            }
        }
        relevant_ids.extend(blockers);

        sorted_goals.retain(|g| relevant_ids.contains(&g.id));
    }

    // Sort goals by stable ID
    sorted_goals.sort_by(|a, b| {
        let id_a = index.meta_to_anchor.get(&a.id);
        let id_b = index.meta_to_anchor.get(&b.id);
        id_a.cmp(&id_b)
    });
    
    for goal in sorted_goals {
        if let Some(anchor_id) = index.meta_to_anchor.get(&goal.id) {
            // Determine status and extract blockers if any
            let (status, blockers) = match &goal.status {
                new_surface_syntax::proof_state::GoalStatus::Unsolved => (crate::goals_panel::GoalStatus::Unsolved, Vec::new()),
                new_surface_syntax::proof_state::GoalStatus::Solved(_) => (crate::goals_panel::GoalStatus::SOLVED, Vec::new()),
                new_surface_syntax::proof_state::GoalStatus::Blocked { depends_on } => {
                     let blocker_infos = depends_on.iter().filter_map(|dep_id| {
                        let dep_goal = id_to_goal.get(dep_id)?;
                        let dep_anchor = index.meta_to_anchor.get(dep_id)?;
                        Some(crate::goals_panel::BlockerInfo {
                            id: dep_anchor.clone(),
                            description: format!("?{}", dep_goal.name),
                        })
                    }).collect();
                    (crate::goals_panel::GoalStatus::Blocked, blocker_infos)
                },
                new_surface_syntax::proof_state::GoalStatus::Inconsistent { .. } => (crate::goals_panel::GoalStatus::Error, Vec::new()),
            };
            
            // Map Span
            let start = crate::span_conversion::offset_to_position(&doc.parsed.text, goal.span.map(|s| s.start).unwrap_or(0));
            let end = crate::span_conversion::offset_to_position(&doc.parsed.text, goal.span.map(|s| s.end).unwrap_or(0));
            
            let range = if let (Some(s), Some(e)) = (start, end) {
                tower_lsp::lsp_types::Range { start: s, end: e }
            } else {
                tower_lsp::lsp_types::Range::default()
            };

            let delta = deltas.get(anchor_id).cloned();
            let summary = Some(compute_structural_summary(goal));

            items.push(crate::goals_panel::GoalPanelItem {
                id: anchor_id.clone(),
                label: format!("?{} : ...", goal.name), // TODO: Pretty print type
                status,
                range,
                blockers,
                delta,
                summary,
            });
        }
    }
    
    Some(crate::goals_panel::GoalsPanelResponse {
        uri: uri.to_string(),
        goals: items,
        version: doc.version,
        stale: false,
        banner,
    })
}
}

// Helper to project ProofState goals into UI Goal structs with stable IDs
fn compute_ui_goals(
    _uri: &Url,
    parsed: &ParsedDocument,
    ps: &new_surface_syntax::proof_state::ProofState,
    index: &GoalsPanelIndex,
) -> Vec<Goal> {
    use new_surface_syntax::diagnostics::ByteSpan;
    // We walk the ProofState goals (which form the stable backbone)
    // and try to correlate them with parsed syntactic info if useful.
    // For now, we generate the Goals primarily from ProofState to ensure
    // the UI is "honest" about what the kernel sees.
    
    // We need to render the context and target.
    // Ideally we use a PrettyPrinter here, but ProofSession doesn't hold one easily.
    // For MVP Phase 4.7, we can use a basic string representation or placeholders.
    // The "target" field is String.
    
    // Iterate sorted stable anchors to maintain order in UI?
    // Or just iterate proof.goals which are sorted in index creation?
    // Index creation sorted them by ID.
    
    // Let's iterate the index's meta_to_anchor to ensure we get all anchored goals.
    
    let mut ui_goals = Vec::new();
    
    for goal in &ps.goals {
        if let Some(anchor_id) = index.meta_to_anchor.get(&goal.id) {
            let span = goal.span.map(|s| crate::document::ByteSpan::new(s.start, s.end))
                .unwrap_or(crate::document::ByteSpan::new(0, 0));
                
            // Generate context binders from goal.local_context
             let context = goal.local_context.entries.iter().map(|entry| {
                crate::document::Binding {
                    name: entry.name.clone(),
                    kind: crate::document::BindingKind::Def, // Simplified for now
                    span: crate::document::ByteSpan::new(0, 0), // No span info handy for bindings in LocalContext yet
                    value_preview: None,
                    ty_preview: None,
                }
            }).collect();
            
            ui_goals.push(Goal {
                goal_id: format!("?{}", goal.name), // Legacy ID for compat
                stable_id: Some(anchor_id.clone()),
                name: Some(goal.name.clone()),
                span,
                context,
                target: "TODO: Render Type".to_string(), // We need PrettyCtx here eventually
            });
        }
    }
    
    
    ui_goals
}

#[cfg(test)]
mod tests {
    use super::*;
    use new_surface_syntax::diagnostics::projection::GoalsPanelIndex;
    use new_surface_syntax::proof_state::{ProofState, MetaSubst, ElaborationTrace, GoalState, HoleOwner, MorMetaId, LocalContext, MorType, ObjExpr, GoalStatus};
    use new_surface_syntax::diagnostics::{DiagnosticContext, ByteSpan};
    use tower_lsp::lsp_types::Url;

    #[test]
    fn test_compute_ui_goals_stability() {
        // 1. Setup ProofState with 1 goal
        let g1 = GoalState {
            id: MorMetaId(1),
            name: "test".to_string(),
            owner: HoleOwner::Def("foo".to_string()),
            ordinal: 0,
            span: Some(source_span::Span::new(10, 20)),
             local_context: LocalContext { entries: vec![], doctrine: None },
            expected_type: MorType { src: ObjExpr::Meta(new_surface_syntax::proof_state::ObjMetaId(1)), dst: ObjExpr::Meta(new_surface_syntax::proof_state::ObjMetaId(2)) },
            status: GoalStatus::Unsolved,
            relevant_constraints: vec![],
        };
        
        let ps = ProofState {
            goals: vec![g1.clone()],
            constraints: vec![],
            subst: MetaSubst::new(),
            trace: ElaborationTrace::new(),
            conflicts: vec![],
            solver_error: None,
            cycles: vec![],
        };
        
        // 2. Create Index
        let ctx = DiagnosticContext::new("test.ml".to_string(), "test.ml");
        let index = GoalsPanelIndex::new(&ps, &ctx);
        
        // 3. Mock ParsedDocument (mostly ignored by current compute_ui_goals implementation)
        // Use parse method to get valid struct
        let parsed = ParsedDocument::parse("(def foo (hole test))".to_string());
        
        // 4. Compute UI goals
        let uri = Url::parse("file:///test.ml").unwrap();
        let ui_goals = compute_ui_goals(&uri, &parsed, &ps, &index);
        
        // 5. Verify Stable ID
        assert_eq!(ui_goals.len(), 1);
        assert!(ui_goals[0].stable_id.is_some());
        let stable_id = ui_goals[0].stable_id.as_ref().unwrap();
        assert!(stable_id.contains(":def/foo:0"));
        assert_eq!(ui_goals[0].name.as_deref(), Some("test"));
        
        // 6. Verify Resolver
        let resolved = index.resolve_anchor(stable_id);
        assert_eq!(resolved.unwrap().0, MorMetaId(1));
    }
}

fn compute_structural_summary(goal: &new_surface_syntax::proof_state::GoalState) -> String {
    let mut parts = Vec::new();
    match &goal.status {
        new_surface_syntax::proof_state::GoalStatus::Unsolved => parts.push("Directly solvable".to_string()),
        new_surface_syntax::proof_state::GoalStatus::Blocked { depends_on } => {
            parts.push(format!("Blocked by {} metas", depends_on.len()));
        }
        _ => {}
    }
    parts.join("; ")
}
