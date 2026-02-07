use std::{sync::Arc, time::Duration};
// Removed async_trait import
use serde::{Deserialize, Serialize};
// Removed serde_json::Value import
use source_span::Span;
use tokio::sync::{mpsc, RwLock};
// Removed tokio::io::AsyncReadExt import
use tokio::process::Command;
use tokio::time::{self, Instant};
use tower_lsp::{
    Client, LanguageServer,
    jsonrpc::Result,
    lsp_types::{
        CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic, DiagnosticSeverity,
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        DidSaveTextDocumentParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
        Hover, HoverContents, HoverParams, InitializeParams, InitializeResult, InitializedParams,
        InlayHint, InlayHintKind, InlayHintParams, MessageType, NumberOrString, OneOf, Position,
        Range, SelectionRange, SelectionRangeParams, ServerCapabilities,
        ServerInfo, TextDocumentContentChangeEvent,
        TextDocumentSyncCapability,
        TextDocumentSyncKind, Url, WorkDoneProgressOptions,
        ExecuteCommandParams, ExecuteCommandOptions,
        SemanticTokensParams, SemanticTokensResult, SemanticTokens, 
        SemanticTokensLegend, SemanticTokensServerCapabilities, SemanticTokensOptions, 
        SemanticTokensFullOptions,
    },
};

use crate::document::{
    Binding, BindingKind, ByteSpan, ParsedDocument, offset_to_position, position_to_offset,
    top_level_symbols,
};
use crate::tactics::{
    ActionSafety, Selection, TacticLimits, TacticRegistry, TacticRequest,
    stdlib::register_std_tactics,
};
use crate::proof_session::{ProofSession, ProofSessionOpenResult, ProofSessionUpdateResult};
// Removed ClientSink imports

use new_surface_syntax::comrade_workspace::WorkspaceReport;
use new_surface_syntax::{
    ContentChange, SurfaceError, WorkspaceDiagnosticSeverity,
    workspace_diagnostic_from_surface_error,
    diagnostics::pretty::{PrinterRegistry, PrettyCtx}, // Added PrettyCtx
};
use sniper_db::SniperDatabase;

const EXTERNAL_COMMAND_TIMEOUT_MS: u64 = 5000;
const DEFAULT_DEBOUNCE_INTERVAL_MS: u64 = 250;
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_EXTERNAL_COMMAND: Option<&str> = None;

// DebouncedDocumentChange moved back to src/lsp.rs
#[derive(Debug)]
pub struct DebouncedDocumentChange {
    pub uri: Url,
    pub version: i32,
    pub content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub debounce_interval_ms: u64,
    pub log_level: String,
    pub external_command: Option<Vec<String>>,
    pub pretty_dialect: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debounce_interval_ms: DEFAULT_DEBOUNCE_INTERVAL_MS,
            log_level: DEFAULT_LOG_LEVEL.to_owned(),
            external_command: DEFAULT_EXTERNAL_COMMAND.map(|s| vec![s.to_owned()]),
            pretty_dialect: None,
        }
    }
}

fn parse_external_output_to_diagnostics(_uri: &Url, output_line: &str) -> Diagnostic {
    let (severity, message) = if output_line.contains("error:") {
        (DiagnosticSeverity::ERROR, output_line.to_string())
    } else if output_line.contains("warning:") {
        (DiagnosticSeverity::WARNING, output_line.to_string())
    } else if output_line.contains("trace/") || output_line.contains("overlap/") {
        (DiagnosticSeverity::INFORMATION, output_line.to_string())
    } else {
        (DiagnosticSeverity::INFORMATION, output_line.to_string())
    };
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 1)),
        severity: Some(severity),
        code: None,
        code_description: None,
        source: Some("ExternalCommand".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

async fn run_external_command_and_parse_diagnostics(
    client: &Client, // Changed client_sink to Client
    uri: &Url,
    command_args: &[String],
) -> Vec<Diagnostic> {
    if command_args.is_empty() {
        return Vec::new();
    }
    let mut cmd = Command::new(&command_args[0]);
    if command_args.len() > 1 {
        cmd.args(&command_args[1..]);
    }
    if let Ok(path) = uri.to_file_path() {
        if let Some(parent) = path.parent() {
            cmd.current_dir(parent);
        }
    }
    let output = time::timeout(
        Duration::from_millis(EXTERNAL_COMMAND_TIMEOUT_MS),
        cmd.output(),
    )
    .await;
    match output {
        Ok(Ok(output)) => {
            let mut diagnostics = Vec::new();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            for line in stdout.lines() {
                if !line.is_empty() {
                    diagnostics.push(parse_external_output_to_diagnostics(uri, line));
                }
            }
            for line in stderr.lines() {
                if !line.is_empty() {
                    diagnostics.push(parse_external_output_to_diagnostics(uri, line));
                }
            }
            diagnostics
        }
        Ok(Err(e)) => {
            client
                .log_message(
                    MessageType::ERROR,
                    format!("Failed to execute external command '{}': {}", command_args[0], e),
                )
                .await;
            vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("ExternalCommand".to_string()),
                message: format!("Failed to execute command: {}", e),
                related_information: None,
                tags: None,
                data: None,
            }]
        }
        Err(_) => {
            client
                .log_message(
                    MessageType::ERROR,
                    format!(
                        "External command '{}' timed out after {}ms",
                        command_args[0], EXTERNAL_COMMAND_TIMEOUT_MS
                    ),
                )
                .await;
            vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("ExternalCommand".to_string()),
                message: format!(
                        "Command timed out after {}ms. Consider increasing 'EXTERNAL_COMMAND_TIMEOUT_MS' or optimizing the command.",
                        EXTERNAL_COMMAND_TIMEOUT_MS
                    ),
                related_information: None,
                tags: None,
                data: None,
            }]
        }
    }
}

fn workspace_report_to_diagnostics(report: &WorkspaceReport, text: &str) -> Vec<Diagnostic> {
    report
        .diagnostics
        .iter()
        .map(|workspace_diag| {
            let span = workspace_diag
                .span
                .unwrap_or_else(|| Span::new(0, text.len()));
            let clamped = clamp_span(span, text.len());
            Diagnostic {
                range: byte_span_to_range(text, ByteSpan::new(clamped.start, clamped.end)),
                severity: Some(workspace_severity_to_lsp(workspace_diag.severity)),
                code: workspace_diag
                    .code
                    .map(|code| NumberOrString::String(code.to_string())),
                code_description: None,
                source: Some("ComradeWorkspace".into()),
                message: workspace_diag.message.clone(),
                related_information: None,
                tags: None,
                data: None,
            }
        })
        .collect()
}

pub fn document_diagnostics_from_report(
    uri: &Url,
    report: &WorkspaceReport,
    parsed_doc: &ParsedDocument,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(parsed_doc.diagnostics.iter().map(|pd| tower_lsp::lsp_types::Diagnostic {
        range: byte_span_to_range(&parsed_doc.text, pd.span),
        severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("parser".to_string()),
        message: pd.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }));
    diagnostics.extend(workspace_report_to_diagnostics(
        report,
        &parsed_doc.text,
    ));
    sort_diagnostics(uri, &mut diagnostics);
    diagnostics
}

fn diagnostic_sort_key<'a>(
    uri: &'a Url,
    diag: &'a Diagnostic,
) -> (
    &'a str,
    u8, // Reliability score: 0 for spanned, 1 for spanless
    u8, // Severity rank
    u32,
    u32,
    u32,
    u32,
    &'a str,
    String,
    &'a str,
) {
    let reliability = if diag.range.start.line == 0 && diag.range.start.character == 0 && diag.range.end.line == 0 && diag.range.end.character == 0 { 1 } else { 0 };
    (
        uri.as_str(),
        reliability,
        severity_rank(diag.severity),
        diag.range.start.line,
        diag.range.start.character,
        diag.range.end.line,
        diag.range.end.character,
        diag.message.as_str(),
        diagnostic_code_to_str(diag.code.as_ref()),
        diagnostic_source_to_str(diag.source.as_ref()),
    )
}

fn diagnostic_code_to_str(code: Option<&NumberOrString>) -> String {
    code.map(|c| match c {
        NumberOrString::Number(n) => n.to_string(),
        NumberOrString::String(s) => s.clone(),
    })
    .unwrap_or_default()
}

fn diagnostic_source_to_str(source: Option<&String>) -> &str {
    source.map(|s| s.as_str()).unwrap_or_default()
}

fn sort_diagnostics(uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.sort_by(|a, b| diagnostic_sort_key(uri, a).cmp(&diagnostic_sort_key(uri, b)));
}

fn workspace_severity_to_lsp(severity: WorkspaceDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        WorkspaceDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
        WorkspaceDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        WorkspaceDiagnosticSeverity::Information => DiagnosticSeverity::INFORMATION,
    }
}

fn severity_rank(severity: Option<DiagnosticSeverity>) -> u8 {
    match severity {
        Some(sev) if sev == DiagnosticSeverity::ERROR => 0,
        Some(sev) if sev == DiagnosticSeverity::WARNING => 1,
        Some(sev) if sev == DiagnosticSeverity::INFORMATION => 2,
        Some(sev) if sev == DiagnosticSeverity::HINT => 3,
        _ => 4,
    }
}

fn clamp_span(span: Span, text_len: usize) -> Span {
    Span::new(span.start.min(text_len), span.end.min(text_len))
}

fn byte_span_to_range(text: &str, span: ByteSpan) -> Range {
    let start = offset_to_position(text, span.start);
    let end = offset_to_position(text, span.end);
    Range::new(start, end)
}

fn chain_to_selection_range(text: &str, chain: &[ByteSpan]) -> SelectionRange {
    let mut current: Option<Box<SelectionRange>> = None;
    for span in chain.iter().rev() {
        let range = byte_span_to_range(text, *span);
        current = Some(Box::new(SelectionRange {
            range,
            parent: current,
        }));
    }
    match current {
        Some(node) => *node,
        None => SelectionRange {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            parent: None,
        },
    }
}



pub fn workspace_error_report(err: &SurfaceError) -> WorkspaceReport {
    WorkspaceReport {
        diagnostics: vec![workspace_diagnostic_from_surface_error(err)],
        fingerprint: None,
        revision: 0,
        bundle: None,
        proof_state: None,
    }
}





pub struct Backend {
    client: Client, // Changed to Client
    proof_session: Arc<RwLock<ProofSession>>,
    config: Arc<RwLock<Config>>,
    did_change_tx: mpsc::Sender<DebouncedDocumentChange>,
    registry: Arc<PrinterRegistry>, // NEW: server-wide printer registry
    tactic_registry: Arc<TacticRegistry>, // NEW: tactic engine
    db: Arc<SniperDatabase>, // SniperDB instance
}

impl Backend {
    pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type
        let (did_change_tx, did_change_rx) = mpsc::channel(100);
        let proof_session_arc = Arc::new(RwLock::new(ProofSession::new(client.clone(), config.clone())));
        
        // Initialize printer registry once at startup
        let registry = Arc::new(PrinterRegistry::new_with_defaults());

        // Initialize tactic registry
        let mut tactic_registry = TacticRegistry::new();
        register_std_tactics(&mut tactic_registry);
        let tactic_registry = Arc::new(tactic_registry);

        // Initialize SniperDB
        let db = Arc::new(SniperDatabase::new());

        tokio::spawn(process_debounced_proof_session_events(
            proof_session_arc.clone(),
            config.clone(), // Pass config directly
            config.clone(), // Pass config directly
            client.clone(),
            db.clone(),
            did_change_rx,
        ));
        Self {
            client,
            proof_session: proof_session_arc,
            config: config, // Use 'config' here
            did_change_tx,
            registry,
            tactic_registry,
            db,
        }
    }
} // End of impl Backend

// async_trait not needed here. It's for LanguageServer trait impl
async fn process_debounced_proof_session_events(
    proof_session_arc: Arc<RwLock<ProofSession>>,
    config_arc: Arc<RwLock<Config>>,
    config_arc: Arc<RwLock<Config>>,
    client: Client, // Changed to Client
    db: Arc<SniperDatabase>,
    mut receiver: mpsc::Receiver<DebouncedDocumentChange>,
) {
    let mut last_change_time: Option<Instant> = None;
    let mut pending_change: Option<DebouncedDocumentChange> = None;
    loop {
        let debounce_interval = Duration::from_millis(
            config_arc
                .read()
                .await
                .debounce_interval_ms,
        );
        let sleep_duration = match last_change_time {
            Some(time) => {
                let elapsed = time.elapsed();
                if elapsed < debounce_interval {
                    debounce_interval - elapsed
                } else {
                    Duration::from_millis(0)
                }
            }
            None => debounce_interval,
        };
        tokio::select! {
            Some(change) = receiver.recv() => {
                pending_change = Some(change);
                last_change_time = Some(Instant::now());
            }
            _ = time::sleep(sleep_duration), if pending_change.is_some() => {
                if let Some(change) = pending_change.take() {
                    client.log_message(
                        MessageType::INFO,
                        format!("Processing debounced change for {} v{}", change.uri, change.version).to_string(),
                    ).await;
                    let ProofSessionUpdateResult {
                        report: _,
                        diagnostics,
                        goals: _,
                    } = {
                        let mut proof_session = proof_session_arc.write().await;
                        proof_session.update(change.uri.clone(), change.version, change.content_changes.clone()).await
                    };

                    // Update SniperDB input
                    // We need to resolve the full text from ProofSession (or cache it locally).
                    // Since ProofSession updates its own parsed document, we can read it back.
                    {
                        let session = proof_session_arc.read().await;
                        if let Some(doc) = session.get_parsed_document(&change.uri) {
                             // TODO: Map URI to FileId. For Stage A, we hash the URI string to get u32?
                             // Or we add a map to SniperDatabase. 
                             // Let's use a simple hash for now to unblock.
                             let file_id = crc32fast::hash(change.uri.as_str().as_bytes());
                             db.set_input(file_id, doc.text.clone());
                        }
                    }
                    
                    client
                        .publish_diagnostics(
                            change.uri.clone(),
                            diagnostics,
                            Some(change.version),
                        )
                        .await;
                }
            }
            else => {
                break;
            }
        }
    }
}

use async_trait::async_trait; // Keep async_trait for LanguageServer trait impl
#[async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let mut config = self.config.write().await;
        if let Some(options) = params.initialization_options.as_ref() {
            match serde_json::from_value::<Config>(options.clone()) {
                Ok(parsed_config) => {
                    *config = parsed_config;
                }
                Err(e) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!("Failed to parse initialization options: {}", e).to_string(),
                        )
                        .await;
                }
            }
        }
        self.client
            .log_message(
                MessageType::INFO,
                format!("Server initialized with config: {:?}", *config).to_string(), // Debug OK
            )
            .await;
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "edgelord-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                selection_range_provider: Some(
                    tower_lsp::lsp_types::SelectionRangeProviderCapability::Simple(true),
                ),
                document_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
                code_action_provider: Some(
                    tower_lsp::lsp_types::CodeActionProviderCapability::Simple(true),
                ),
                inlay_hint_provider: Some(OneOf::Left(true)),
                diagnostic_provider: Some(
                    tower_lsp::lsp_types::DiagnosticServerCapabilities::Options(
                        tower_lsp::lsp_types::DiagnosticOptions {
                            identifier: Some("edgelord-lsp".into()),
                            inter_file_dependencies: false,
                            workspace_diagnostics: false,
                            work_done_progress_options: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                        },
                    ),
                ),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["edgelord/goals".to_string(), "edgelord/explain".to_string()],
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                            legend: SemanticTokensLegend {
                                token_types: crate::highlight::LEGEND_TOKEN_TYPES.to_vec(),
                                token_modifiers: crate::highlight::LEGEND_TOKEN_MODIFIERS.to_vec(),
                            },
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        }
                    )
                ),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "edgelord-lsp initialized".to_string())
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;
        let ProofSessionOpenResult {
            report: _,
            diagnostics,
            goals: _,
        } = self.proof_session.write().await.open(uri.clone(), version, text.clone()).await;
        
        // Update SniperDB
        let file_id = crc32fast::hash(uri.as_str().as_bytes());
        self.db.set_input(file_id, text);

        self.client
            .publish_diagnostics(
                uri,
                diagnostics,
                Some(version),
            )
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let change = DebouncedDocumentChange {
            uri: params.text_document.uri,
            version: params.text_document.version,
            content_changes: params.content_changes,
        };
        if let Err(e) = self.did_change_tx.send(change).await {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("Failed to send didChange event to debouncer: {}", e).to_string(),
                )
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = self.proof_session.read().await.get_document_version(&uri).unwrap_or(0);
        let mut diagnostics_to_publish = Vec::new();
        let proof_session_diagnostics = {
            let mut proof_session = self.proof_session.write().await;
            if let Some(text) = params.text {
                let result = proof_session.open(uri.clone(), version, text).await;
                result.diagnostics
            } else {
                let result = proof_session.apply_command(uri.clone(), "re-analyze-on-save".to_string()).await;
                result.diagnostics
            }
        };
        diagnostics_to_publish.extend(proof_session_diagnostics);

        let config = self.config.read().await;
        if let Some(cmd_args) = config.external_command.clone() {
            if !cmd_args.is_empty() {
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "Running external command on save for {}: {:?}", // Debug OK
                            uri, cmd_args
                        ).to_string(),
                    )
                    .await;
                let external_diagnostics =
                    run_external_command_and_parse_diagnostics(&self.client, &uri, &cmd_args)
                        .await;
                diagnostics_to_publish.extend(external_diagnostics);
            }
        }
        drop(config);

        sort_diagnostics(&uri, &mut diagnostics_to_publish);
        self.client
            .publish_diagnostics(
                uri.clone(),
                diagnostics_to_publish,
                Some(version),
            )
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.proof_session.write().await.close(&uri);
        self.client
            .publish_diagnostics(
                uri,
                Vec::new(),
                None,
            )
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let markdown_content_option = {
            let session = self.proof_session.read().await;
            if let Some(proof) = session.get_proof_state(&uri) {
                 let offset = position_to_offset(&session.get_parsed_document(&uri).unwrap().text, position);
                 if let Some(goal) = proof.goals.iter().find(|g| g.span.map_or(false, |s| s.start <= offset && offset <= s.end)) {
                    // Create pretty context
                    let config = self.config.read().await;
                    let dialect = config.pretty_dialect.as_deref()
                        .map(|s| match s {
                            "pythonic" => new_surface_syntax::diagnostics::pretty::PrettyDialect::Pythonic,
                             "canonical" => new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
                            _ => new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
                        })
                        .unwrap_or(new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical);
                    
                    let files = new_surface_syntax::diagnostics::DiagnosticContext::new("hover_dummy".to_string(), ""); // Dummy for now, ideally populated
                    let ctx = crate::edgelord_pretty_ctx::EdgeLordPrettyCtx::new(
                        &self.registry,
                        dialect,
                        new_surface_syntax::diagnostics::pretty::PrettyLimits::hover_default(),
                        proof,
                        &files,
                        &uri,
                    );

                    // Render goal view
                    let view = ctx.printer().render_goal(&ctx, goal);
                    
                    // Format markdown
                    let mut md = format!("### {}\n\n**Status**: {}\n\n", view.title, view.status_line);
                    for detail in view.details {
                        md.push_str(&format!("- {}\n", detail));
                    }
                    
                    // Version footer
                    let version = session.get_document_version(&uri).unwrap_or(0);
                    md.push_str(&format!("\n\n---\n_v{}_", version));
                    
                    Some(md)
                } else {
                    None
                }
            } else {
                None
            }

        };
        Ok(markdown_content_option.map(|md| Hover {
            contents: HoverContents::Markup(tower_lsp::lsp_types::MarkupContent {
                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                value: md,
            }),
            range: None,
        }))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>> {
        if params.command == "edgelord/goals" {
            let args = params.arguments;
            if args.is_empty() {
                 return Err(tower_lsp::jsonrpc::Error::invalid_params("Missing arguments"));
            }
            // Expecting first argument to be { "textDocument": { "uri": "..." } } or just "uri_string"
            // Let's support an object with textDocument field for standard compliance, 
            // or a simple string for ease of testing.
            
            let uri_str: String = if let Some(obj) = args[0].as_object() {
                if let Some(td) = obj.get("textDocument") {
                    td.get("uri").and_then(|v| v.as_str()).map(|s| s.to_string())
                        .ok_or(tower_lsp::jsonrpc::Error::invalid_params("Missing uri in textDocument"))?
                } else if let Some(u) = obj.get("uri") {
                     u.as_str().map(|s| s.to_string())
                         .ok_or(tower_lsp::jsonrpc::Error::invalid_params("Missing uri field"))?
                } else {
                     return Err(tower_lsp::jsonrpc::Error::invalid_params("Invalid argument structure"));
                }
            } else if let Some(s) = args[0].as_str() {
                s.to_string()
            } else {
                return Err(tower_lsp::jsonrpc::Error::invalid_params("Invalid argument type"));
            };
            
            let uri = Url::parse(&uri_str)
                .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI format"))?;
                
            let response = self.proof_session.read().await.compute_goals_panel(&uri);
            
            match response {
                Some(resp) => {
                    let json = serde_json::to_value(resp).map_err(|e| {
                         tower_lsp::jsonrpc::Error {
                             code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                             message: format!("Serialization error: {}", e).into(),
                             data: None
                         }
                    })?;
                    Ok(Some(json))
                },
                None => Ok(None) // or error if document not found?
            }
        } else if params.command == "edgelord/explain" {
            // Parse ExplainRequest from arguments
            let args = params.arguments;
            if args.is_empty() {
                return Err(tower_lsp::jsonrpc::Error::invalid_params("Missing arguments"));
            }
            
            // Deserialize ExplainRequest from first argument
            let req: crate::explain::view::ExplainRequest = serde_json::from_value(args[0].clone())
                .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid ExplainRequest: {}", e)))?;
            
            // Call handler
            let view = crate::explain::handle_explain_request(req, self.proof_session.clone()).await?;
            
            // Serialize response
            let json = serde_json::to_value(view).map_err(|e| {
                tower_lsp::jsonrpc::Error {
                    code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                    message: format!("Failed to serialize explain response: {}", e).into(),
                    data: None,
                }
            })?;
            
            Ok(Some(json))
        } else {
            Ok(None)
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;

        let session = self.proof_session.read().await;
        let doc = session
            .get_document(&uri)
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Document not found"))?;

        let proof_state = doc.workspace_report.proof_state.as_ref();
        if proof_state.is_none() {
            return Ok(None);
        }
        let proof_state = proof_state.unwrap();

        let dialect = self.config.read().await.pretty_dialect.as_deref()
            .map(|s| match s {
                "pythonic" => new_surface_syntax::diagnostics::pretty::PrettyDialect::Pythonic,
                _ => new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
            })
            .unwrap_or(new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical);

        let files = new_surface_syntax::diagnostics::DiagnosticContext::new(uri.to_string(), "");
        let pretty_ctx = crate::edgelord_pretty_ctx::EdgeLordPrettyCtx::new(
            &self.registry,
            dialect,
            new_surface_syntax::diagnostics::pretty::PrettyLimits::hover_default(),
            proof_state,
            &files,
            &uri,
        );

        let req = TacticRequest {
            ctx: &pretty_ctx,
            proof: proof_state,
            doc: &doc.parsed,
            index: doc.goals_index.as_ref(),
            selection: Selection { range },
            limits: TacticLimits::default(),
        };

        let actions = self.tactic_registry.compute_all(&req);

        let mut response: Vec<_> = actions
            .into_iter()
            .map(|a| {
                CodeActionOrCommand::CodeAction(tower_lsp::lsp_types::CodeAction {
                    title: a.title,
                    kind: Some(map_tactic_kind(a.kind)),
                    diagnostics: None,
                    edit: Some(a.edit),
                    command: None,
                    is_preferred: Some(a.safety == ActionSafety::Safe),
                    disabled: None,
                    data: None,
                })
            })
            .collect();

        // Add Loogle lemma suggestions
        let loogle_actions = crate::loogle::generate_loogle_actions(
            session.loogle_index(),
            proof_state,
            range,
            &uri,
        );
        
        response.extend(
            loogle_actions
                .into_iter()
                .map(CodeActionOrCommand::CodeAction)
        );

        Ok(Some(response))
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        let ranges = {
            let proof_session = self.proof_session.read().await;
            if let Some(doc) = proof_session.get_parsed_document(&uri) {
                Some(
                    params
                        .positions
                        .iter()
                        .map(|position| {
                            let offset = position_to_offset(&doc.text, *position);
                            let chain = doc.selection_chain_for_offset(offset);
                            chain_to_selection_range(&doc.text, &chain)
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        };
        Ok(ranges)
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        
        let hints_result = {
            let session = self.proof_session.read().await;
            if let Some(proof) = session.get_proof_state(&uri) {
                // Initialize context
                let config = self.config.read().await;
                let dialect = config.pretty_dialect.as_deref()
                        .map(|s| match s {
                            "pythonic" => new_surface_syntax::diagnostics::pretty::PrettyDialect::Pythonic,
                             "canonical" => new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
                            _ => new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical,
                        })
                        .unwrap_or(new_surface_syntax::diagnostics::pretty::PrettyDialect::Canonical);
                
                let files = new_surface_syntax::diagnostics::DiagnosticContext::new("hover_dummy".to_string(), "");
                let ctx = crate::edgelord_pretty_ctx::EdgeLordPrettyCtx::new(
                    &self.registry,
                    dialect,
                    new_surface_syntax::diagnostics::pretty::PrettyLimits::inlay_default(),
                    proof,
                    &files,
                    &uri,
                );

                let mut hints = Vec::new();
                let doc_text = &session.get_parsed_document(&uri).unwrap().text;
                let start_offset = position_to_offset(doc_text, range.start);
                let end_offset = position_to_offset(doc_text, range.end);

                for goal in &proof.goals {
                     if let Some(span) = goal.span {
                         if span.start >= start_offset && span.end <= end_offset {
                             let view = ctx.printer().render_goal(&ctx, goal);
                             
                             let position = offset_to_position(doc_text, span.end);
                             hints.push(InlayHint {
                                     position,
                                     label: tower_lsp::lsp_types::InlayHintLabel::String(format!(": {}", view.title)),
                                     kind: Some(InlayHintKind::TYPE),
                                     text_edits: None,
                                     tooltip: Some(tower_lsp::lsp_types::InlayHintTooltip::String(view.status_line)),
                                     padding_left: Some(true),
                                     padding_right: Some(true),
                                     data: None,
                                 });
                         }
                     }
                }
                Some(hints)
            } else {
                None
            }
        };
        Ok(hints_result)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let tokens = {
            let session = self.proof_session.read().await;
            if let Some(doc) = session.get_document(&uri) {
                let text = &doc.parsed.text;
                // Compute tokens using highlight module (Layer 0 + 1)
                let mut raw_tokens = crate::highlight::compute_layer0_structural(text);
                
                // Convert to LSP format
                let data = crate::highlight::tokens_to_lsp_data(text, &mut raw_tokens);
                Some(SemanticTokens {
                    result_id: None,
                    data,
                })
            } else {
                None
            }
        };
        Ok(tokens.map(SemanticTokensResult::Tokens))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let symbols = {
            let proof_session = self.proof_session.read().await;
            if let Some(doc) = proof_session.get_parsed_document(&uri) {
                Some(
                    top_level_symbols(&doc.text)
                        .into_iter()
                        .map(|(name, span)| {
                            let range = byte_span_to_range(&doc.text, span);
                            DocumentSymbol {
                                name,
                                detail: None,
                                kind: tower_lsp::lsp_types::SymbolKind::FUNCTION,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: range,
                                children: None,
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        };
        Ok(symbols.map(DocumentSymbolResponse::Nested))
    }

    async fn diagnostic(
        &self,
        params: tower_lsp::lsp_types::DocumentDiagnosticParams,
    ) -> Result<tower_lsp::lsp_types::DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        let items = self.proof_session.read().await.get_diagnostics(&uri);

        Ok(
            tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
                tower_lsp::lsp_types::DocumentDiagnosticReport::Full(
                    tower_lsp::lsp_types::RelatedFullDocumentDiagnosticReport {
                        related_documents: None,
                        full_document_diagnostic_report:
                            tower_lsp::lsp_types::FullDocumentDiagnosticReport {
                                result_id: None,
                                items,
                            },
                    },
                ),
            ),
        )
    }
}

fn map_tactic_kind(kind: crate::tactics::view::ActionKind) -> tower_lsp::lsp_types::CodeActionKind {
    use crate::tactics::view::ActionKind;
    match kind {
        ActionKind::QuickFix => tower_lsp::lsp_types::CodeActionKind::QUICKFIX,
        ActionKind::Refactor => tower_lsp::lsp_types::CodeActionKind::REFACTOR,
        ActionKind::Rewrite => tower_lsp::lsp_types::CodeActionKind::REFACTOR_REWRITE,
        ActionKind::Explain => tower_lsp::lsp_types::CodeActionKind::new("edgelord.explain"),
        ActionKind::Expand => tower_lsp::lsp_types::CodeActionKind::REFACTOR_INLINE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParsedDocument;
    use new_surface_syntax::comrade_workspace::WorkspaceReport;
    use new_surface_syntax::{WorkspaceDiagnostic, WorkspaceDiagnosticSeverity};
    fn sample_report() -> WorkspaceReport {
        WorkspaceReport {
            diagnostics: vec![
                WorkspaceDiagnostic {
                    message: "beta".into(),
                    span: Some(Span::new(0, 1)),
                    severity: WorkspaceDiagnosticSeverity::Warning,
                    code: Some("macro"),
                    notes: vec![],
                },
                WorkspaceDiagnostic {
                    message: "alpha".into(),
                    span: Some(Span::new(1, 2)),
                    severity: WorkspaceDiagnosticSeverity::Error,
                    code: Some("parse"),
                    notes: vec![],
                },
            ],
            fingerprint: None,
            revision: 0,
            bundle: None,
            proof_state: None,
        }
    }
    #[test]
    fn workspace_report_to_diagnostics_is_deterministic() {
        let report = sample_report();
        let uri = Url::parse("file:///test.tld").unwrap(); // Dummy URL for testing
        let mut first = workspace_report_to_diagnostics(&report, "012");
        sort_diagnostics(&uri, &mut first);
        let mut second = workspace_report_to_diagnostics(&report, "012");
        sort_diagnostics(&uri, &mut second);
        assert_eq!(first, second, "sorting + conversion must be repeatable");
    }
    #[test]
    fn document_diagnostics_include_workspace_report_diag() {
        let report = WorkspaceReport {
            diagnostics: vec![WorkspaceDiagnostic::error(
                "workspace diag",
                Some(Span::new(0, 1)),
                Some("code"),
            )],
            fingerprint: None,
            revision: 0,
            bundle: None,
            proof_state: None,
        };
        let parsed = ParsedDocument::parse("(touch x)".to_string());
        let uri = Url::parse("file:///test.tld").unwrap(); // Dummy URL for testing
        let diagnostics = document_diagnostics_from_report(&uri, &report, &parsed); // Pass uri
        assert!(
            diagnostics.iter().any(|diag| diag.message == "workspace diag"),
            "workspace diagnostics must be preserved in every LSP report"
        );
    }
    #[test]
    fn test_extract_trace_steps_v1() {
        let term = "(quote (trace-v1 (version 1) (meta (foo bar)) (steps (step1 arg1) (step2) (step3 argA argB))))";
        let steps = extract_trace_steps(term);
        assert_eq!(steps, Some(vec!["(step1".to_string(), "arg1)".to_string(), "(step2)".to_string(), "(step3".to_string(), "argA".to_string(), "argB".to_string()]));
    }
    #[tokio::test]
    async fn test_extract_trace_steps_legacy() {
        let term = "(quote (step_a step_b (compound step)))";
        let steps = extract_trace_steps(term);
        assert_eq!(steps, Some(vec!["step_a".to_string(), "step_b".to_string(), "(compound".to_string(), "step)".to_string()]));
    }
    #[test]
    fn test_extract_trace_steps_no_match() {
        let term = "(some-other-quote (data (abc)))";
        let steps = extract_trace_steps(term);
        assert_eq!(steps, None);
    }
}