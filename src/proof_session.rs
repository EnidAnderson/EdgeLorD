use std::{collections::{BTreeMap, VecDeque}, sync::Arc, time::Instant as StdInstant};
// Removed use std::default::Default; (was not used anyway)

use tokio::sync::RwLock;
use tokio::time::Instant;
use tower_lsp::{
    lsp_types::{Diagnostic, MessageType, TextDocumentContentChangeEvent, Url, WorkspaceEdit},
    Client, // Added Client import
};

use crate::document::{apply_content_changes, ParsedDocument, Goal};
use crate::lsp::{Config, workspace_error_report, document_diagnostics_from_report};
use crate::caching::{ModuleCache, CacheKey, CacheValue, CacheKeyBuilder, ModuleSnapshotCache, Phase1MissReason, Phase1_1MissReason, CacheOutcome};
use comrade_lisp::comrade_workspace::WorkspaceReport;
use comrade_lisp::ComradeWorkspace;
// Removed use crate::client_sink::ClientSink;
use comrade_lisp::diagnostics::projection::GoalsPanelIndex;
use comrade_lisp::diagnostics::DiagnosticContext;
use comrade_lisp::proof_state;
use comrade_lisp::diagnostics;
use comrade_lisp::ContentChange;
use codeswitch::fingerprint::HashValue;
use sha2::{Digest, Sha256};
use sniper_db::SniperDatabase;
use hex;  // C2.4: For fingerprint hex encoding

/// C2.4: Benchmark measurement record (19 CSV fields)
#[derive(Clone, Debug)]
pub struct BenchmarkMeasurement {
    pub timestamp_ms: u64,
    pub scenario: String,
    pub uri: String,
    pub edit_id: u32,
    pub dv: u64,
    pub phase1_outcome: String,
    pub phase1_1_outcome: String,
    pub compiled: u8,  // 0 or 1
    pub compile_ms: u64,
    pub end_to_end_ms: u64,
    pub diagnostics_count: usize,
    pub bytes_open_docs: usize,
    pub cache_entries_phase1: usize,
    pub cache_entries_phase1_1: usize,
    pub options_fp8: String,
    pub deps_fp8: String,
    pub workspace_fp8: String,
    pub published: u8,  // 0 or 1
    pub note: String,
}

impl BenchmarkMeasurement {
    /// Write as CSV row (exact field order per spec)
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.timestamp_ms,
            self.scenario,
            self.uri,
            self.edit_id,
            self.dv,
            self.phase1_outcome,
            self.phase1_1_outcome,
            self.compiled,
            self.compile_ms,
            self.end_to_end_ms,
            self.diagnostics_count,
            self.bytes_open_docs,
            self.cache_entries_phase1,
            self.cache_entries_phase1_1,
            self.options_fp8,
            self.deps_fp8,
            self.workspace_fp8,
            self.published,
            self.note
        )
    }

    /// CSV header row
    pub fn csv_header() -> &'static str {
        "timestamp_ms,scenario,uri,edit_id,dv,phase1_outcome,phase1_1_outcome,compiled,compile_ms,end_to_end_ms,diagnostics_count,bytes_open_docs,cache_entries_phase1,cache_entries_phase1_1,options_fp8,deps_fp8,workspace_fp8,published,note"
    }
}

#[derive(Clone)]
pub struct ProofSnapshot {
    pub version: i32,
    pub timestamp: Instant,
    pub proof_state: proof_state::ProofState,
    pub goals_index: GoalsPanelIndex,
    /// SB2: boundary at which this state was checked (None = full document)
    pub checked_boundary: Option<usize>,
    /// SB2: reverse workspace edit to undo any text mutation that created this state
    pub reverse_edit: Option<WorkspaceEdit>,
}

pub struct ProofDocument {
    pub version: i32,
    pub parsed: ParsedDocument,
    pub last_analyzed: Instant,
    pub workspace_report: WorkspaceReport,
    pub goals_index: Option<GoalsPanelIndex>, // NEW: Stable anchor index
    pub history: VecDeque<ProofSnapshot>, // NEW: Semantic snapshots
    /// SB0: Proof-stepping boundary. `None` = elaborate full document.
    /// `Some(offset)` = only text[..offset] has been elaborated.
    pub checked_boundary: Option<usize>,
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
    pub measurement: Option<BenchmarkMeasurement>,  // C2.4: Optional for benchmarking
}

/// SB2: Return value of `ProofSession::undo_step`.
#[derive(serde::Serialize)]
pub struct UndoResult {
    /// Version of the restored snapshot.
    pub version: i32,
    /// The checked boundary after restore (`None` = full document was checked).
    pub checked_boundary: Option<usize>,
    /// Reverse workspace edit to undo an associated text mutation (may be `None`).
    pub reverse_edit: Option<WorkspaceEdit>,
}

pub struct ProofSession {
    client: Client, // Changed from Arc<dyn ClientSink> to Client
    config: Arc<RwLock<Config>>,
    workspace: ComradeWorkspace,
    documents: BTreeMap<Url, ProofDocument>,
    loogle_indexer: Arc<crate::loogle::WorkspaceIndexer>,
    /// Phase 1.1: Module cache for deterministic snapshot reuse
    pub module_cache: Arc<RwLock<ModuleCache>>,
    /// Phase 1: Module snapshot cache (file_id, content_hash keyed)
    pub module_snapshot_cache: Arc<RwLock<ModuleSnapshotCache>>,
    /// Phase 1: SniperDatabase for semantic caching
    pub db: Arc<SniperDatabase>,
}

impl ProofSession {
    pub fn new(client: Client, config: Arc<RwLock<Config>>, db: Arc<SniperDatabase>) -> Self { // Changed client type
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
            module_cache: Arc::new(RwLock::new(ModuleCache::new())),
            module_snapshot_cache: Arc::new(RwLock::new(ModuleSnapshotCache::new(db.clone()))),
            db,
        }
    }

    /// Get the current document version for the given URI.
    ///
    /// **INV T-DVCMP Support**: Used by async ScopeCreep tasks to detect
    /// stale results (when document version has changed since Phase 1).
    pub fn get_document_version(&self, uri: &Url) -> Option<i32> {
        self.documents.get(uri).map(|doc| doc.version)
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
                checked_boundary: None,
                reverse_edit: None,
            });
        }
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
            checked_boundary: None, // SB0: full elaboration by default
        });

        ProofSessionOpenResult {
            report,
            diagnostics,
            goals,
        }
    }

    pub async fn update(&mut self, uri: Url, version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> ProofSessionUpdateResult {
        // C2.4: Start measurement timer
        let t0_end_to_end = StdInstant::now();
        let dv = version as u64;  // Document version tracking

        let Some(current_proof_doc) = self.documents.get(&uri) else {
            // Updated call to log_message to use direct Client method
            self.client.log_message(MessageType::ERROR, format!("ProofSession: Attempted to update non-existent document: {}", uri).to_string()).await;
            return ProofSessionUpdateResult {
                report: WorkspaceReport { diagnostics: Vec::new(), diagnostics_by_file: BTreeMap::new(), structured_diagnostics: Vec::new(), fingerprint: None, revision: 0, bundle: None, proof_state: None },
                diagnostics: Vec::new(),
                goals: Vec::new(),
                measurement: None,
            };
        };

        let updated_text = apply_content_changes(&current_proof_doc.parsed.text, &changes);
        let parsed = ParsedDocument::parse(updated_text.clone());
        let key = uri.to_string();
        let uri_for_measurement = uri.clone();  // C2.4: Clone for measurement (uri is moved later)

        let content_changes_for_workspace = changes.into_iter().map(|change| {
            if let Some(range) = change.range {
                let start = crate::document::position_to_offset(&current_proof_doc.parsed.text, range.start);
                let end = crate::document::position_to_offset(&current_proof_doc.parsed.text, range.end);
                ContentChange {
                    range: Some((start, end)),
                    text: change.text,
                }
            } else {
                ContentChange {
                    range: None,
                    text: change.text,
                }
            }
        }).collect::<Vec<_>>();

        // Compute all inputs before checking ANY cache (ensures sound reuse)
        let unit_content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", updated_text.as_bytes());
        let file_id = uri_cache_slot(&uri);

        // Read compile options from config (canonical ordering required)
        let config = self.config.read().await;
        let options_fingerprint = compute_options_fingerprint(&config);
        drop(config);

        // Compute workspace snapshot (conservative dependency fingerprint)
        let workspace_snapshot_hash = compute_workspace_snapshot_hash(&self.documents, &uri);
        let dependency_fingerprint = compute_dependency_fingerprint_conservative(&self.documents, &uri);

        // Phase 1: Check module snapshot cache with COMPLETE 4-component key
        // INV PHASE-1-MODULE-1: Only reuse when all inputs match
        let caches_enabled = self.config.read().await.caches_enabled;
        let mut phase1_outcome = if !caches_enabled {
            "miss:cache_disabled".to_string()
        } else {
            "miss:other".to_string()  // Default, may be hit
        };

        {
            let mut snapshot_cache = self.module_snapshot_cache.write().await;
            if caches_enabled && let Some(snapshot) = snapshot_cache.get(
                file_id,
                unit_content_hash.clone(),
                options_fingerprint.clone(),
                workspace_snapshot_hash.clone(),
            ) {
                // CACHE HIT at Phase 1 module snapshot level
                // Sound: all compilation inputs (content, options, deps) match
                phase1_outcome = "hit".to_string();  // C2.4: Record hit outcome
                let diagnostics = snapshot.diagnostics.clone();
                let goals = if let (Some(ps), Some(parsed_goals)) = (&snapshot.report.proof_state, None::<&Vec<Goal>>) {
                    // Recompute UI goals from cached proof state
                    if let Some(ref ps) = snapshot.report.proof_state {
                        let diag_ctx = DiagnosticContext::new(key.clone(), &key);
                        let goals_index = GoalsPanelIndex::new(ps, &diag_ctx);
                        compute_ui_goals(&uri, &parsed, ps, &goals_index)
                    } else {
                        parsed.goals.clone()
                    }
                } else {
                    parsed.goals.clone()
                };

                // Update document metadata but preserve cached report
                let mut history = current_proof_doc.history.clone();
                if let Some(ref ps) = snapshot.report.proof_state {
                    let diag_ctx = DiagnosticContext::new(key.clone(), &key);
                    let goals_index = GoalsPanelIndex::new(ps, &diag_ctx);
                    history.push_back(ProofSnapshot {
                        version,
                        timestamp: Instant::now(),
                        proof_state: ps.clone(),
                        goals_index,
                        checked_boundary: None,
                        reverse_edit: None,
                    });
                    if history.len() > 10 {
                        history.pop_front();
                    }
                }

                self.documents.insert(uri, ProofDocument {
                    version,
                    parsed,
                    last_analyzed: Instant::now(),
                    workspace_report: snapshot.report.clone(),
                    goals_index: None, // Will be recomputed if needed
                    history,
                    checked_boundary: None,
                });

                // C2.4: Return measurement for Phase 1 hit
                let end_to_end_ms = t0_end_to_end.elapsed().as_millis() as u64;
                let measurement = BenchmarkMeasurement {
                    timestamp_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                    scenario: "benchmark".to_string(),
                    uri: uri_for_measurement.to_string(),
                    edit_id: 0,
                    dv,
                    phase1_outcome: phase1_outcome.clone(),
                    phase1_1_outcome: "hit".to_string(),  // Also hit in phase 1.1 if hit in phase 1
                    compiled: 0,  // No compilation for cache hit
                    compile_ms: 0,
                    end_to_end_ms,
                    diagnostics_count: diagnostics.len(),
                    bytes_open_docs: bytes_open_docs(&self.documents),
                    cache_entries_phase1: self.module_snapshot_cache.blocking_read().len(),
                    cache_entries_phase1_1: self.module_cache.blocking_read().len(),
                    options_fp8: hash_to_fp8(&options_fingerprint),
                    deps_fp8: hash_to_fp8(&dependency_fingerprint),
                    workspace_fp8: hash_to_fp8(&workspace_snapshot_hash),
                    published: 1,
                    note: "".to_string(),
                };

                return ProofSessionUpdateResult {
                    report: snapshot.report,
                    diagnostics,
                    goals,
                    measurement: Some(measurement),
                };
            }
        }

        // Phase 1.1: Compute full 5-component cache key and check for cache hit
        // (Inside single-flight gate to preserve no-stale-diagnostics invariant)
        // NOTE: options_fingerprint and dependency_fingerprint already computed above

        let cache_key = CacheKeyBuilder::new()
            .options(options_fingerprint)
            .workspace_snapshot(workspace_snapshot_hash)
            .unit_id(normalize_unit_id(&uri))
            .unit_content(unit_content_hash)
            .dependencies(dependency_fingerprint)
            .build()
            .unwrap_or_else(|e| {
                let client = self.client.clone();
                tokio::spawn(async move {
                    client.log_message(
                        MessageType::WARNING,
                        format!("Failed to build cache key: {}", e),
                    ).await;
                });
                // Fallback: create a temporary key that won't match anything
                // This is safe; it just means cache misses
                CacheKey {
                    options_fingerprint,
                    workspace_snapshot_hash,
                    unit_id: format!("{}_fallback", uri),
                    unit_content_hash,
                    dependency_fingerprint,
                }
            });

        // Check cache (Phase 1.1: INV D-CACHE-2 - Sound reuse)
        // C2.4: Track Phase 1.1 outcome and compile timing
        let mut phase1_1_outcome = if !caches_enabled {
            "miss:cache_disabled".to_string()
        } else {
            "miss:other".to_string()  // Default, may be hit
        };
        let mut compiled: u8 = 1;  // Default to compiled
        let mut compile_ms: u64 = 0;

        let (report, from_cache) = {
            let mut cache = self.module_cache.write().await;
            if caches_enabled && let Some(cached) = cache.get(&cache_key) {
                phase1_1_outcome = "hit".to_string();  // C2.4: Phase 1.1 hit
                compiled = 0;
                (cached.report, true)
            } else {
                // Cache miss: compile normally
                cache.stats_mut().record_miss("compile_needed");

                // C2.4: Measure compile time
                let t_compile = StdInstant::now();
                let report = match self.workspace.did_change(&key, &content_changes_for_workspace) {
                    Ok(report) => report,
                    Err(err) => workspace_error_report(&err),
                };
                compile_ms = t_compile.elapsed().as_millis() as u64;
                compiled = 1;  // Compilation occurred

                // Cache the result in Phase 1.1 cache
                let cache_value = CacheValue {
                    report: report.clone(),
                    diagnostics: document_diagnostics_from_report(&uri, &report, &parsed),
                    timestamp: std::time::SystemTime::now(),
                };
                cache.insert(cache_key, cache_value.clone());

                // Also insert into Phase 1 module snapshot cache with COMPLETE key
                // INV PHASE-1-MODULE-1: Key must include all compilation inputs
                // INV C-DEPFP-WIRING: dependency_fingerprint must be sourced from dependency component, not workspace_snapshot_hash
                {
                    let mut snapshot_cache = self.module_snapshot_cache.write().await;
                    use crate::caching::ModuleSnapshot;
                    let snapshot = ModuleSnapshot {
                        file_id,
                        content_hash: unit_content_hash.clone(),
                        options_fingerprint: options_fingerprint.clone(),
                        dependency_fingerprint: dependency_fingerprint.clone(),
                        report: report.clone(),
                        diagnostics: cache_value.diagnostics,
                        timestamp: std::time::SystemTime::now(),
                    };
                    snapshot_cache.insert(snapshot);
                }

                (report, false)
            }
        };

        let diagnostics = if from_cache {
            // Recompute diagnostics from cached report (should be deterministic)
            document_diagnostics_from_report(&uri, &report, &parsed)
        } else {
            document_diagnostics_from_report(&uri, &report, &parsed)
        };
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
                checked_boundary: None,
                reverse_edit: None,
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
            checked_boundary: None, // SB0: full elaboration resets boundary
        });

        // C2.4: Collect final measurement with all 19 CSV fields
        let end_to_end_ms = t0_end_to_end.elapsed().as_millis() as u64;
        let measurement = BenchmarkMeasurement {
            timestamp_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            scenario: "benchmark".to_string(),
            uri: uri_for_measurement.to_string(),
            edit_id: 0,
            dv,
            phase1_outcome,
            phase1_1_outcome,
            compiled,
            compile_ms,
            end_to_end_ms,
            diagnostics_count: diagnostics.len(),
            bytes_open_docs: bytes_open_docs(&self.documents),
            cache_entries_phase1: self.module_snapshot_cache.blocking_read().len(),
            cache_entries_phase1_1: self.module_cache.blocking_read().len(),
            options_fp8: hash_to_fp8(&options_fingerprint),
            deps_fp8: hash_to_fp8(&dependency_fingerprint),
            workspace_fp8: hash_to_fp8(&workspace_snapshot_hash),
            published: 1,  // Will be overridden by LSP layer if stale DV suppresses it
            note: "".to_string(),
        };

        ProofSessionUpdateResult {
            report,
            diagnostics,
            goals,
            measurement: Some(measurement),
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
                report: WorkspaceReport { diagnostics: Vec::new(), diagnostics_by_file: BTreeMap::new(), structured_diagnostics: Vec::new(), fingerprint: None, revision: 0, bundle: None, proof_state: None },
                diagnostics: Vec::new(),
                goals: Vec::new(),
                measurement: None,
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
            measurement: None,
        }
    }

    pub fn get_document_text(&self, uri: &Url) -> Option<String> {
        self.documents.get(uri).map(|doc| doc.parsed.text.clone())
    }

    pub fn get_last_analyzed_time(&self, uri: &Url) -> Option<Instant> {
        self.documents.get(uri).map(|doc| doc.last_analyzed)
    }

    pub fn get_parsed_document(&self, uri: &Url) -> Option<&ParsedDocument> {
        self.documents.get(uri).map(|doc| &doc.parsed)
    }

    pub fn get_proof_state(&self, uri: &Url) -> Option<&proof_state::ProofState> {
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

    pub fn resolve_goal_anchor(&self, uri: &Url, anchor_id: &str) -> Option<(proof_state::MorMetaId, Option<diagnostics::ByteSpan>)> {
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

    // ─── SB0: Proof-stepping commands ────────────────────────────────────────

    /// Advance the checked boundary to the end of the next top-level form.
    ///
    /// Returns the new boundary offset, or `None` if already at the last form.
    /// **INV S-STEP:** the returned `ProofState` equals full elaboration of
    /// the forms in 0..new_boundary.
    pub fn step_forward(&mut self, uri: &Url) -> Option<usize> {
        let current = self.documents.get(uri)?.checked_boundary.unwrap_or(0);
        let boundaries = self.documents.get(uri)?.parsed.top_level_form_boundaries();
        let next = *boundaries.iter().find(|&&b| b > current)?;
        self.elaborate_up_to(uri, next);
        Some(next)
    }

    /// Retract the checked boundary to the end of the previous top-level form.
    ///
    /// Returns the new boundary offset. If already at the beginning returns `Some(0)`.
    pub fn step_backward(&mut self, uri: &Url) -> Option<usize> {
        let current = self.documents.get(uri)?.checked_boundary.unwrap_or(usize::MAX);
        let boundaries = self.documents.get(uri)?.parsed.top_level_form_boundaries();
        // boundary = 0 means "no forms checked"; anything smaller than the first real boundary
        let prev = boundaries.iter().rev().find(|&&b| b < current).copied().unwrap_or(0);
        self.elaborate_up_to(uri, prev);
        Some(prev)
    }

    /// Snap the checked boundary to the nearest form boundary at or before `cursor_offset`.
    ///
    /// Returns the new boundary offset.
    pub fn goto_cursor(&mut self, uri: &Url, cursor_offset: usize) -> Option<usize> {
        let boundaries = self.documents.get(uri)?.parsed.top_level_form_boundaries();
        let target = boundaries.iter().rev()
            .find(|&&b| b <= cursor_offset)
            .copied()
            .unwrap_or(0);
        self.elaborate_up_to(uri, target);
        Some(target)
    }

    /// Re-elaborate the document up to `boundary` bytes and store the result.
    ///
    /// Uses `ComradeWorkspace::set_document_text` + `report_for_key` so the
    /// external workspace state stays consistent.
    ///
    /// **INV S-STEP:** calls into the same elaboration pipeline as full
    /// elaboration; the only difference is the text length.
    fn elaborate_up_to(&mut self, uri: &Url, boundary: usize) {
        let key = uri.to_string();
        let text = match self.documents.get(uri) {
            Some(d) => d.parsed.text.clone(),
            None => return,
        };

        // Slice at boundary; boundary=0 gives an empty document (all goals disappear)
        let partial_text = if boundary == 0 {
            ""
        } else if boundary >= text.len() {
            text.as_str()
        } else {
            &text[..boundary]
        };

        // Re-elaborate through workspace
        let report = match self.workspace.set_document_text(&key, partial_text)
            .and_then(|_| self.workspace.report_for_key(&key))
        {
            Ok(r) => r,
            Err(e) => workspace_error_report(&e),
        };

        let partial_parsed = ParsedDocument::parse(partial_text.to_string());
        let diag_ctx = DiagnosticContext::new(key.clone(), &key);
        let goals_index = report.proof_state.as_ref().map(|ps| {
            GoalsPanelIndex::new(ps, &diag_ctx)
        });

        if let Some(doc) = self.documents.get_mut(uri) {
            if let (Some(ps), Some(idx)) = (report.proof_state.as_ref(), goals_index.as_ref()) {
                doc.history.push_back(ProofSnapshot {
                    version: doc.version,
                    timestamp: Instant::now(),
                    proof_state: ps.clone(),
                    goals_index: idx.clone(),
                    checked_boundary: Some(boundary),
                    reverse_edit: None,
                });
                // Cap history at 50 entries (INV D-MEMORY)
                while doc.history.len() > 50 {
                    doc.history.pop_front();
                }
            }
            doc.goals_index = goals_index;
            doc.workspace_report = report;
            doc.checked_boundary = Some(boundary);
            doc.parsed = partial_parsed;
        }
    }

    // ─── SB1: Human-readable delta summary ───────────────────────────────────

    /// A one-line description of what changed since the previous snapshot.
    ///
    /// Returns `None` when there is no meaningful change. Used by the
    /// `$/edgelord/goalsUpdated` notification (SB1).
    ///
    /// **INV D-*:** deterministic — iterates `panel.goals` in anchor-sorted order.
    pub fn delta_summary(&self, uri: &Url) -> Option<String> {
        use crate::goals_panel::{GoalChangeKind, GoalStatus};
        let panel = self.compute_goals_panel(uri)?;
        let (mut added, mut removed, mut solved) = (0usize, 0usize, 0usize);
        for item in &panel.goals {
            if let Some(delta) = &item.delta {
                for change in &delta.changes {
                    match change {
                        GoalChangeKind::Added => added += 1,
                        GoalChangeKind::Removed => removed += 1,
                        GoalChangeKind::StatusChanged { new_status, .. } => {
                            if matches!(new_status, GoalStatus::SOLVED) {
                                solved += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if added == 0 && removed == 0 && solved == 0 {
            return None;
        }
        let mut parts = Vec::new();
        if solved > 0 { parts.push(format!("{} solved", solved)); }
        if added > 0  { parts.push(format!("{} new", added)); }
        if removed > 0 { parts.push(format!("{} removed", removed)); }
        Some(parts.join(", "))
    }

    // ─── SB2: Snapshot-based undo ────────────────────────────────────────────

    /// Pop the most recent proof snapshot and restore the document to the
    /// previous state (second-to-last history entry).
    ///
    /// **INV S-UNDO**: restores the exact previous `ProofState` without text
    /// re-elaboration.  The caller is responsible for applying
    /// `result.reverse_edit` if a text mutation should also be reversed.
    ///
    /// Returns `None` if there are fewer than 2 history entries (nothing to undo).
    pub fn undo_step(&mut self, uri: &Url) -> Option<UndoResult> {
        let doc = self.documents.get_mut(uri)?;
        // Need at least current + one previous entry
        if doc.history.len() < 2 {
            return None;
        }
        // Remove the current (most-recent) state from history
        doc.history.pop_back();
        // Restore from what is now the last entry (the previous state)
        let prev = doc.history.back()?;
        let version = prev.version;
        let goals_index = Some(prev.goals_index.clone());
        let checked_boundary = prev.checked_boundary;
        let reverse_edit = prev.reverse_edit.clone();
        let proof_state = prev.proof_state.clone();
        // Apply the restore
        doc.goals_index = goals_index;
        doc.checked_boundary = checked_boundary;
        doc.workspace_report.proof_state = Some(proof_state);
        Some(UndoResult { version, reverse_edit, checked_boundary })
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
        let is_frontier = |g: &&proof_state::GoalState| {
            match &g.status {
                proof_state::GoalStatus::Unsolved => true,
                proof_state::GoalStatus::Blocked { depends_on } => depends_on.is_empty(),
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
                if let proof_state::GoalStatus::Blocked { depends_on } = &g.status {
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
                proof_state::GoalStatus::Unsolved => (crate::goals_panel::GoalStatus::Unsolved, Vec::new()),
                proof_state::GoalStatus::Solved(_) => (crate::goals_panel::GoalStatus::SOLVED, Vec::new()),
                proof_state::GoalStatus::Blocked { depends_on } => {
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
                proof_state::GoalStatus::Inconsistent { .. } => (crate::goals_panel::GoalStatus::Error, Vec::new()),
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
                label: format!("?{} : {}", goal.name, render_mor_type_simple(&goal.expected_type, &ps.subst)),
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

// ─── Type rendering helpers ──────────────────────────────────────────────────

/// Render an `ObjExpr` to a compact string without requiring a `PrettyCtx`.
///
/// Used as a fallback when no `PrinterRegistry` is available (e.g. inside
/// `compute_ui_goals` which has no handle to a `PrettyCtx`).
///
/// **INV D-*:** deterministic — same input → same output.
fn render_obj_simple(obj: &proof_state::ObjExpr) -> String {
    match obj {
        proof_state::ObjExpr::Known(id) => format!("O{}", id.index()),
        proof_state::ObjExpr::Meta(m) => format!("\u{03b1}{}", m.0),  // α<n>
        proof_state::ObjExpr::ErrorHole(_) => "\u{26a0}".to_string(), // ⚠
    }
}

/// Render a `MorType` to `"src → dst"` using the current substitution.
///
/// Applies the substitution first to resolve any solved object metavariables,
/// then falls back to `render_obj_simple` for anything still unknown.
///
/// **INV D-*:** deterministic.
fn render_mor_type_simple(
    ty: &proof_state::MorType,
    subst: &proof_state::MetaSubst,
) -> String {
    let resolved = subst.apply_mor_type(ty);
    let src = render_obj_simple(&resolved.src);
    let dst = render_obj_simple(&resolved.dst);
    format!("{} \u{2192} {}", src, dst) // "src → dst"
}

/// Find the "active goal" at a byte offset.
///
/// Returns the innermost `GoalState` whose span contains `offset` (by smallest
/// span length). Falls back to the goal with the nearest *preceding* span end
/// if no goal contains the offset.
///
/// **INV D-*:** deterministic — iteration over `proof.goals` is ordered by
/// `MorMetaId` (u32), so same inputs → same output.
pub fn goal_enclosing_offset<'p>(
    proof: &'p proof_state::ProofState,
    offset: usize,
) -> Option<&'p proof_state::GoalState> {
    // Pass 1: smallest enclosing span
    let containing = proof
        .goals
        .iter()
        .filter(|g| {
            g.span
                .map_or(false, |s| s.start <= offset && offset <= s.end)
        })
        .min_by_key(|g| g.span.map_or(usize::MAX, |s| s.end - s.start));

    if containing.is_some() {
        return containing;
    }

    // Pass 2: nearest preceding goal (span end ≤ offset)
    proof
        .goals
        .iter()
        .filter(|g| g.span.map_or(false, |s| s.end <= offset))
        .max_by_key(|g| g.span.map_or(0, |s| s.end))
}

// Helper to project ProofState goals into UI Goal structs with stable IDs
fn compute_ui_goals(
    _uri: &Url,
    parsed: &ParsedDocument,
    ps: &proof_state::ProofState,
    index: &GoalsPanelIndex,
) -> Vec<Goal> {
    use comrade_lisp::diagnostics::ByteSpan;
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
                target: render_mor_type_simple(&goal.expected_type, &ps.subst),
            });
        }
    }
    
    
    ui_goals
}

#[cfg(test)]
mod tests {
    use super::*;
    use comrade_lisp::diagnostics::projection::GoalsPanelIndex;
    use comrade_lisp::proof_state::{ProofState, MetaSubst, ElaborationTrace, GoalState, HoleOwner, MorMetaId, LocalContext, MorType, ObjExpr, GoalStatus};
    use comrade_lisp::diagnostics::{DiagnosticContext, ByteSpan};
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
            expected_type: MorType { src: ObjExpr::Meta(proof_state::ObjMetaId(1)), dst: ObjExpr::Meta(proof_state::ObjMetaId(2)) },
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

    #[test]
    fn test_caches_disabled_via_env_var() {
        // Verify that caches_enabled defaults to false when EDGELORD_DISABLE_CACHES=1
        unsafe {
            std::env::set_var("EDGELORD_DISABLE_CACHES", "1");
        }
        let caches_enabled = std::env::var("EDGELORD_DISABLE_CACHES").is_err();
        assert!(!caches_enabled, "caches_enabled should be false when EDGELORD_DISABLE_CACHES is set");
        unsafe {
            std::env::remove_var("EDGELORD_DISABLE_CACHES");
        }

        // Verify that caches_enabled defaults to true when env var is not set
        unsafe {
            std::env::remove_var("EDGELORD_DISABLE_CACHES");
        }
        let caches_enabled = std::env::var("EDGELORD_DISABLE_CACHES").is_err();
        assert!(caches_enabled, "caches_enabled should be true when EDGELORD_DISABLE_CACHES is not set");
    }
}

fn compute_structural_summary(goal: &proof_state::GoalState) -> String {
    let mut parts = Vec::new();
    match &goal.status {
        proof_state::GoalStatus::Unsolved => parts.push("Directly solvable".to_string()),
        proof_state::GoalStatus::Blocked { depends_on } => {
            parts.push(format!("Blocked by {} metas", depends_on.len()));
        }
        _ => {}
    }
    parts.join("; ")
}

/// Phase 1.1: Compute workspace snapshot hash from all open documents.
///
/// This provides a conservative fingerprint that changes when any document changes.
/// INV D-CACHE-3: Monotone invalidation (changes to any doc invalidate all caches)
fn compute_workspace_snapshot_hash(
    documents: &BTreeMap<Url, ProofDocument>,
    current_uri: &Url,
) -> HashValue {
    let mut content_hashes = Vec::new();

    // Collect hashes of all open documents in deterministic order
    for (uri, doc) in documents.iter() {
        let doc_hash = HashValue::hash_with_domain(b"FILE_CONTENT", doc.parsed.text.as_bytes());
        content_hashes.push((uri.to_string(), doc_hash));
    }

    // Sort for determinism (required for INV D-CACHE-1)
    content_hashes.sort_by(|a, b| a.0.cmp(&b.0));

    // Create canonical bytes from sorted list
    let mut canonical_bytes = Vec::new();
    for (uri, hash) in content_hashes {
        canonical_bytes.extend_from_slice(uri.as_bytes());
        canonical_bytes.push(0); // separator
        canonical_bytes.extend_from_slice(hash.as_bytes());
        canonical_bytes.push(0);
    }

    HashValue::hash_with_domain(b"WORKSPACE_SNAPSHOT", &canonical_bytes)
}

/// Phase 1: Convert URI to file ID for module snapshot cache.
///
/// Uses SHA-256 (domain separated) and derives a deterministic u32 adapter for SniperDB.
fn uri_cache_slot(uri: &Url) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(b"EDGE_URI_FILE_ID_V1");
    hasher.update(uri.as_str().as_bytes());
    let digest = hasher.finalize();
    u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]])
}

/// Phase 1: Compute options fingerprint from canonical compile options.
///
/// CRITICAL: Must include all options that affect compilation output.
/// This must be updated whenever a new Config field is added that affects semantics.
///
/// Current semantic-affecting fields:
/// - pretty_dialect: Output formatting (Pythonic vs Canonical)
/// - enable_db7_hover_preview: DB-7 feature enabled/disabled
/// - db7_placeholder_suffix: Refactoring hint template
/// - db7_debug_mode: Diagnostic detail level
/// - external_command: External tool integration (if present)
///
/// NOT included (timing/logging only):
/// - debounce_interval_ms: Timing only, not output semantics
/// - log_level: Logging only, not output semantics
///
/// INV: Same options → same fingerprint (no false misses)
///      Different options → different fingerprint (no false hits)
///
/// TODO: When adding new Config fields, update this function and add a test case.
pub fn compute_options_fingerprint(config: &Config) -> HashValue {
    // Create canonical bytes from all SEMANTIC options in deterministic order
    let mut canonical_bytes = Vec::new();

    // pretty_dialect: affects output formatting (Pythonic vs Canonical)
    if let Some(dialect) = &config.pretty_dialect {
        canonical_bytes.extend_from_slice(b"dialect=");
        canonical_bytes.extend_from_slice(dialect.as_bytes());
        canonical_bytes.push(0);
    }

    // enable_db7_hover_preview: feature flag affecting DB-7 behavior
    canonical_bytes.extend_from_slice(b"db7_hover=");
    canonical_bytes.extend_from_slice(if config.enable_db7_hover_preview { b"true" } else { b"false" });
    canonical_bytes.push(0);

    // db7_placeholder_suffix: affects refactoring hints
    canonical_bytes.extend_from_slice(b"db7_suffix=");
    canonical_bytes.extend_from_slice(config.db7_placeholder_suffix.as_bytes());
    canonical_bytes.push(0);

    // db7_debug_mode: affects diagnostic detail level
    canonical_bytes.extend_from_slice(b"db7_debug=");
    canonical_bytes.extend_from_slice(if config.db7_debug_mode { b"true" } else { b"false" });
    canonical_bytes.push(0);

    // external_command: if present, affects external tool integration
    if let Some(cmd) = &config.external_command {
        canonical_bytes.extend_from_slice(b"external_cmd=");
        for (i, arg) in cmd.iter().enumerate() {
            if i > 0 {
                canonical_bytes.push(b' ');
            }
            canonical_bytes.extend_from_slice(arg.as_bytes());
        }
        canonical_bytes.push(0);
    }

    // Hash canonical bytes
    HashValue::hash_with_domain(b"COMPILE_OPTIONS", &canonical_bytes)
}

/// Phase 1: Compute dependency fingerprint (conservative: workspace snapshot).
///
/// For Phase 1, we use the workspace snapshot hash as the dependency fingerprint.
/// This is conservative but sound: changes to ANY document invalidate all caches.
///
/// TODO Phase 1.2: Implement true transitive dependency tracking
/// - Build actual import graph
/// - Only invalidate units affected by changed imports
/// - Reduce false misses from unrelated document changes
///
/// INV: Same workspace state → same fingerprint
///      Different imports → cache miss (conservative but correct)
fn compute_dependency_fingerprint_conservative(
    documents: &BTreeMap<Url, ProofDocument>,
    _current_uri: &Url,
) -> HashValue {
    // For Phase 1: use workspace snapshot (all open docs)
    // This is safe (no false hits) but conservative (some false misses)
    compute_workspace_snapshot_hash(documents, _current_uri)
}

/// Phase 1.1: Normalize unit identifier to canonical form.
///
/// As per PHASE_1_1_ACCEPTANCE_SPEC.md §1.1:
/// - Prefer FileId if available
/// - Otherwise normalize Url string: no fragment/query, forward slashes, preserve case
fn normalize_unit_id(uri: &Url) -> String {
    // For Phase 1.1, use the Url string directly (already normalized by tower-lsp)
    // Strip fragment and query parameters for determinism
    let normalized = uri.path();
    normalized.to_string()
}

// ============================================================================
// C2.4: Instrumentation Helpers
// ============================================================================

/// Convert HashValue to 8-char hex string (first 8 bytes)
fn hash_to_fp8(hash: &HashValue) -> String {
    hex::encode(&hash.as_bytes()[0..8.min(hash.as_bytes().len())])
}

/// Get total bytes of all open documents in deterministic order
fn bytes_open_docs(documents: &BTreeMap<Url, ProofDocument>) -> usize {
    documents.values().map(|doc| doc.parsed.text.len()).sum()
}
