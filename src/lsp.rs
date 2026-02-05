use std::{sync::Arc, time::Duration, collections::HashMap};
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
        PublishDiagnosticsParams, Range, SelectionRange, SelectionRangeParams, ServerCapabilities,
        ServerInfo, TextDocumentContentChangeEvent,
        TextDocumentSyncCapability,
        TextDocumentSyncKind, Url, WorkDoneProgressOptions,
    },
};

use crate::document::{
    Binding, BindingKind, ByteSpan, ParsedDocument, offset_to_position, position_to_offset,
    top_level_symbols,
};
use crate::proof_session::{ProofSession, ProofSessionOpenResult, ProofSessionUpdateResult};
// Removed ClientSink imports

use new_surface_syntax::comrade_workspace::WorkspaceReport;
use new_surface_syntax::{
    ComradeWorkspace, ContentChange, SurfaceError, WorkspaceDiagnosticSeverity,
    workspace_diagnostic_from_surface_error,
};

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debounce_interval_ms: DEFAULT_DEBOUNCE_INTERVAL_MS,
            log_level: DEFAULT_LOG_LEVEL.to_owned(),
            external_command: DEFAULT_EXTERNAL_COMMAND.map(|s| vec![s.to_owned()]),
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
    u8,
    u32,
    u32,
    u32,
    u32,
    &'a str,
    String,
    &'a str,
) {
    (
        uri.as_str(),
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

fn format_context(bindings: &[Binding], max_items: usize) -> String {
    if bindings.is_empty() {
        return "(empty)".to_string();
    }
    let shown = bindings
        .iter()
        .take(max_items)
        .map(|b| format!("{} {}", binding_kind_label(b.kind), b.name))
        .collect::<Vec<_>>();
    if bindings.len() > max_items {
        format!(
            "{}, … +{} more",
            shown.join(", "),
            bindings.len() - max_items
        )
    } else {
        shown.join(", ")
    }
}

fn binding_kind_label(kind: BindingKind) -> &'static str {
    match kind {
        BindingKind::Let => "let",
        BindingKind::Touch => "touch",
        BindingKind::Def => "def",
    }
}

pub fn workspace_error_report(err: &SurfaceError) -> WorkspaceReport {
    WorkspaceReport {
        diagnostics: vec![workspace_diagnostic_from_surface_error(err)],
        fingerprint: None,
        revision: 0,
        bundle: None,
    }
}

fn lsp_changes_to_content_changes(
    base_text: &str,
    events: &[TextDocumentContentChangeEvent],
) -> (Vec<ContentChange>, String) {
    let mut current_text = base_text.to_string();
    let mut content_changes = Vec::with_capacity(events.len());
    for change in events {
        if let Some(range) = change.range {
            let start = position_to_offset(&current_text, range.start).min(current_text.len());
            let end = position_to_offset(&current_text, range.end).min(current_text.len());
            let start = start.min(end);
            let end = start.max(end);
            content_changes.push(ContentChange {
                range: Some((start, end)),
                text: change.text.clone(),
            });
            current_text.replace_range(start..end, &change.text);
        } else {
            content_changes.push(ContentChange {
                range: None,
                text: change.text.clone(),
            });
            current_text = change.text.clone();
        }
    }
    (content_changes, current_text)
}

fn extract_trace_steps(term: &str) -> Option<Vec<String>> {
    if let Some(steps_start) = term.find("(steps ") {
        let steps_content = &term[steps_start + "(steps ".len()..];
        if let Some(steps_end) = steps_content.find(")))") {
            let steps_str = &steps_content[..steps_end];
            let steps: Vec<String> = steps_str
                .split_whitespace()
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if !steps.is_empty() {
                return Some(steps);
            }
        }
    }

    if term.starts_with("(quote (") && term.ends_with("))") {
        let content_start = "(quote (".len();
        let content_end = term.len() - "))".len();
        let content = &term[content_start..content_end];
        let steps: Vec<String> = content
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if !steps.is_empty() {
            return Some(steps);
        }
    }

    None
}

pub struct Backend {
    client: Client, // Changed to Client
    proof_session: Arc<RwLock<ProofSession>>,
    config: Arc<RwLock<Config>>,
    did_change_tx: mpsc::Sender<DebouncedDocumentChange>,
}

impl Backend {
    pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type
        let (did_change_tx, did_change_rx) = mpsc::channel(100);
        let proof_session_arc = Arc::new(RwLock::new(ProofSession::new(client.clone(), config.clone())));
        tokio::spawn(process_debounced_proof_session_events(
            proof_session_arc.clone(),
            config.clone(), // Pass config directly
            client.clone(),
            did_change_rx,
        ));
        Self {
            client,
            proof_session: proof_session_arc,
            config: config, // Use 'config' here
            did_change_tx,
        }
    }
} // End of impl Backend

// async_trait not needed here. It's for LanguageServer trait impl
async fn process_debounced_proof_session_events(
    proof_session_arc: Arc<RwLock<ProofSession>>,
    config_arc: Arc<RwLock<Config>>,
    client: Client, // Changed to Client
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
                        proof_session.update(change.uri.clone(), change.version, change.content_changes).await
                    };
                    
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
                format!("Server initialized with config: {:?}", *config).to_string(),
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
        } = self.proof_session.write().await.open(uri.clone(), version, text).await;
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
                            "Running external command on save for {}: {:?}",
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
        let markdown_content_option: Option<String> = {
            let proof_session = self.proof_session.read().await;
            if let Some(doc) = proof_session.get_parsed_document(&uri) {
                let offset = position_to_offset(&doc.text, position);
                let mut content = String::new();
                if let Some(goal) = doc.goal_at_offset(offset) {
                    let goal_name = goal.name.as_deref().unwrap_or("?");
                    let ctx = format_context(&goal.context, 8);
                    content.push_str(&format!(
                        "**Goal** `{}`\n\n- id: `{}`\n- target: `{}`\n- context: {}",
                        goal_name, goal.goal_id, goal.target, ctx
                    ));
                }
                if let Some(span) = doc.selection_chain_for_offset(offset).first() {
                    if !content.is_empty() {
                        content.push_str("\n\n---\n\n");
                    }
                    content.push_str(&format!("Focused span: [{}..{}]", span.start, span.end));
                }
                if content.is_empty() {
                    None
                } else {
                    let last_analyzed_duration = proof_session.get_last_analyzed_time(&uri)
                                                    .map(|t| t.elapsed().as_millis())
                                                    .unwrap_or(0);
                    let version = proof_session.get_document_version(&uri).unwrap_or(0);
                    let footer = format!("\n\n---\n\n_Document Version: {}, Last Analyzed: {}ms ago_", version, last_analyzed_duration);
                    content.push_str(&footer);
                    Some(content)
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

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let actions = {
            let proof_session = self.proof_session.read().await;
            if let Some(doc) = proof_session.get_parsed_document(&uri) {
                let text = &doc.text;
                let start_offset = position_to_offset(text, range.start);
                let end_offset = position_to_offset(text, range.end);
                let selected_text = text.get(start_offset..end_offset).ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("invalid range"))?;
                let new_text = format!("(quote {})", selected_text);
                let text_edit = tower_lsp::lsp_types::TextEdit {
                    range,
                    new_text,
                };
                let mut changes = HashMap::new();
                changes.insert(uri.clone(), vec![text_edit]);
                let edit = tower_lsp::lsp_types::WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                };
                let code_action = tower_lsp::lsp_types::CodeAction {
                    title: "Wrap in (quote ...)".to_string(),
                    kind: Some(tower_lsp::lsp_types::CodeActionKind::REFACTOR_REWRITE),
                    diagnostics: None,
                    edit: Some(edit),
                    command: None,
                    is_preferred: Some(false),
                    disabled: None,
                    data: None,
                };
                Some(vec![CodeActionOrCommand::CodeAction(code_action)])
            } else {
                None
            }
        };
        Ok(actions)
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
        let hints = {
            let proof_session = self.proof_session.read().await;
            if let Some(doc) = proof_session.get_parsed_document(&uri) {
                let start = position_to_offset(&doc.text, params.range.start);
                let end = position_to_offset(&doc.text, params.range.end);
                let query = ByteSpan::new(start.min(end), start.max(end));
                Some(
                    doc.goal_inlay_hints_in_range(query)
                        .into_iter()
                        .map(|hint| InlayHint {
                            position: offset_to_position(&doc.text, hint.offset),
                            label: hint.label.into(),
                            kind: Some(InlayHintKind::TYPE),
                            text_edits: None,
                            tooltip: None,
                            padding_left: Some(true),
                            padding_right: Some(false),
                            data: None,
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        };
        Ok(hints)
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
                },
                WorkspaceDiagnostic {
                    message: "alpha".into(),
                    span: Some(Span::new(1, 2)),
                    severity: WorkspaceDiagnosticSeverity::Error,
                    code: Some("parse"),
                },
            ],
            fingerprint: None,
            revision: 0,
            bundle: None,
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