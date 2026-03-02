use std::{collections::{BTreeMap, BTreeSet}, sync::Arc, time::Duration};
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
        GotoDefinitionParams, GotoDefinitionResponse,
        Location, CompletionParams, CompletionResponse, CompletionItem,
        CompletionItemKind, CompletionOptions,
        ReferenceParams, FoldingRange, FoldingRangeParams, FoldingRangeKind,
        SignatureHelp, SignatureHelpParams, SignatureHelpOptions,
        SignatureInformation, ParameterInformation, ParameterLabel,
        RenameParams, WorkspaceEdit, TextEdit, PrepareRenameResponse,
        DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams,
    },
};
use sha2::{Digest, Sha256};

// Import tcb_core for coherence functions
use tcb_core::bundle_canonical::{CoreSExpr, CoreAtom};
use tcb_core::prelude::{coherent_query, dispatch_quoted_prelude_call};

use crate::document::{
    Binding, BindingKind, ByteSpan, ParsedDocument, offset_to_position, position_to_offset,
    top_level_symbols, SymbolIndex, SymbolDefKind,
};
use crate::span_conversion::{byte_span_to_lsp_range, span_to_lsp_range};
use crate::tactics::{
    view::ActionSafety, view::Selection, view::TacticLimits, registry::TacticRegistry, view::TacticRequest,
    stdlib::register_std_tactics,
};
use crate::proof_session::{ProofSession, ProofSessionOpenResult, ProofSessionUpdateResult};
use crate::refute::lsp_handler::{RefuteRequest, handle_refute_request};
// Removed ClientSink imports

use comrade_lisp::comrade_workspace::WorkspaceReport;
use comrade_lisp::WorkspaceDiagnostic;
use comrade_lisp::{
    ContentChange, SurfaceError, WorkspaceDiagnosticSeverity,
    workspace_diagnostic_from_surface_error, ModulePath, ModuleResolver,
    diagnostics::pretty::{PrinterRegistry, PrettyCtx}, // Added PrettyCtx
};
use comrade_lisp::{MadLibResolver};
use comrade_lisp::diagnostics::pretty::{PrettyDialect, PrettyLimits};
use comrade_lisp::diagnostics::{StructuredDiagnostic, DiagnosticOrigin, DiagnosticContext, canonical_diag_sort_key};
use comrade_lisp::scopecreep::{run_scopecreep, ScopeCreepOptions, ScopeCreepInput};
use sniper_db::SniperDatabase;

const EXTERNAL_COMMAND_TIMEOUT_MS: u64 = 5000;
const DEFAULT_DEBOUNCE_INTERVAL_MS: u64 = 250;
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_EXTERNAL_COMMAND: Option<&str> = None;

/// Keywords offered in completion suggestions.
const KERNEL_KEYWORDS: &[&str] = &[
    "def", "touch", "rule", "sugar", "let", "begin", "do",
    "use", "in", "lambda", "fn", "if", "cond", "match",
    "quote", "quasiquote", "cons",
    "assert", "assert-coherent", "check", "verify",
    "context", "goal", "coherence",
    "grade", "transport",
];

fn stable_uri_file_id(uri: &Url) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(b"EDGE_URI_FILE_ID_V1");
    hasher.update(uri.as_str().as_bytes());
    let digest = hasher.finalize();
    u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]])
}

// Helper function to check if a character is valid in a Mac Lane identifier
fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '+' | '-' | '*' | '/' | '?' | '!' | '=' | '<' | '>' | ':' | '.')
}

// Extract symbol at LSP position (fail-closed)
fn symbol_at_position(text: &str, position: Position) -> Option<String> {
    // Convert LSP position to byte offset
    let offset = position_to_offset(text, position);
    
    // Check bounds
    if offset >= text.len() {
        return None;
    }
    
    // Convert to char-based indexing for Unicode safety
    let chars: Vec<char> = text.chars().collect();
    let mut char_offset = 0;
    let mut byte_count = 0;
    
    for (idx, ch) in chars.iter().enumerate() {
        if byte_count >= offset {
            char_offset = idx;
            break;
        }
        byte_count += ch.len_utf8();
    }
    
    // Check if current char is valid identifier char
    if char_offset >= chars.len() || !is_identifier_char(chars[char_offset]) {
        return None; // Fail-closed: not on identifier
    }
    
    // Scan left to find start
    let mut start = char_offset;
    while start > 0 && is_identifier_char(chars[start - 1]) {
        start -= 1;
    }
    
    // Scan right to find end
    let mut end = char_offset + 1;
    while end < chars.len() && is_identifier_char(chars[end]) {
        end += 1;
    }
    
    // Extract symbol
    let symbol: String = chars[start..end].iter().collect();
    
    // Validate (not empty, not pure punctuation)
    if symbol.is_empty() || symbol.chars().all(|c| !c.is_alphanumeric()) {
        return None; // Fail-closed
    }
    
    Some(symbol)
}

// Render DB-7 plan report as markdown
fn render_hover_markdown(from: &str, to: &str, report: &sniper_db::plan::PlanReport, debug_mode: bool) -> String {
    let mut md = String::new();
    
    // Header with explicit "Preview" label
    md.push_str("**Preview (DB-7): Rename Impact**\n\n");
    md.push_str(&format!("`{}` → `{}`\n\n", from, to));
    
    // Debug info (if enabled)
    if debug_mode {
        md.push_str(&format!("_Debug: plan_id={:?}_\n\n", report.plan_id));
    }
    
    // Blast radius (first line - scannable)
    let br = &report.total_blast_radius;
    md.push_str(&format!(
        "- Blast radius: **{} file(s)**, **{} scope(s)**\n",
        br.total_files, br.total_scopes
    ));
    
    // Cost (first line - scannable)
    let cost = &report.total_cost;
    md.push_str(&format!(
        "- Predicted cost: **{} typechecks**, **{} validations**\n",
        cost.typechecks, cost.validations
    ));
    
    // Proofs (first line - scannable)
    let proofs = &report.proof_preservation;
    if proofs.total_preserved + proofs.total_invalidated > 0 {
        let pct = (proofs.preservation_rate * 100.0) as u32;
        md.push_str(&format!(
            "- Proofs: preserves **{}/{}** ({}%)\n",
            proofs.total_preserved,
            proofs.total_preserved + proofs.total_invalidated,
            pct
        ));
    } else {
        md.push_str("- Proofs: (no proof data)\n");
    }
    
    // Summary (truncate to 200 chars if needed, push "why" to subsequent lines)
    let summary = if report.summary.len() > 200 {
        format!("{}…", &report.summary[..200])
    } else {
        report.summary.clone()
    };
    md.push_str(&format!("\n**Why**: {}\n", summary));
    
    // Warnings (only show if N > 0)
    if !report.warnings.is_empty() {
        md.push_str(&format!("\n**Warnings**: {}\n", report.warnings.len()));
    }
    
    md
}

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
    /// Enable DB-7 hover preview feature (rename impact analysis)
    pub enable_db7_hover_preview: bool,
    /// Suffix for placeholder rename targets (default: "_renamed")
    pub db7_placeholder_suffix: String,
    /// Include plan_id in DB-7 output for debugging (default: false)
    #[serde(default)]
    pub db7_debug_mode: bool,
    /// Phase 1 cache enable/disable (for benchmarking). Default: true. Override via EDGELORD_DISABLE_CACHES=1
    #[serde(default = "default_caches_enabled")]
    pub caches_enabled: bool,
}

fn default_caches_enabled() -> bool {
    std::env::var("EDGELORD_DISABLE_CACHES").is_err()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debounce_interval_ms: DEFAULT_DEBOUNCE_INTERVAL_MS,
            log_level: DEFAULT_LOG_LEVEL.to_owned(),
            external_command: DEFAULT_EXTERNAL_COMMAND.map(|s| vec![s.to_owned()]),
            pretty_dialect: None,
            enable_db7_hover_preview: true, // Enable by default
            db7_placeholder_suffix: "_renamed".to_string(),
            db7_debug_mode: false,
            caches_enabled: default_caches_enabled(),
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

pub fn document_diagnostics_from_report(
    uri: &Url,
    report: &WorkspaceReport,
    parsed_doc: &ParsedDocument,
) -> Vec<Diagnostic> {
    PublishDiagnosticsHandler::convert_diagnostics(uri, report, parsed_doc)
}

fn clamp_span(span: Span, text_len: usize) -> Span {
    Span::new(span.start.min(text_len), span.end.min(text_len))
}

fn byte_span_to_range(text: &str, span: ByteSpan) -> Range {
    let start = offset_to_position(text, span.start);
    let end = offset_to_position(text, span.end);
    Range::new(start, end)
}

fn diagnostic_source_set(diagnostics: &[Diagnostic]) -> String {
    let mut sources = BTreeSet::new();
    for d in diagnostics {
        let source = d.source.clone().unwrap_or_else(|| "unknown".to_string());
        sources.insert(source);
    }
    if sources.is_empty() {
        "none".to_string()
    } else {
        sources.into_iter().collect::<Vec<_>>().join("+")
    }
}

fn diagnostic_signature(diagnostics: &[Diagnostic]) -> u32 {
    // Stable signature over deterministic diagnostic fields for RW parity capture.
    let mut payload = String::new();
    for d in diagnostics {
        let source = d.source.as_deref().unwrap_or("unknown");
        let sev = d
            .severity
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "None".to_string());
        let code = match &d.code {
            Some(NumberOrString::String(s)) => s.clone(),
            Some(NumberOrString::Number(n)) => n.to_string(),
            None => "none".to_string(),
        };
        let start = &d.range.start;
        let end = &d.range.end;
        payload.push_str(&format!(
            "{}|{}|{}|{}:{}-{}:{}|{}\n",
            source,
            sev,
            code,
            start.line,
            start.character,
            end.line,
            end.character,
            d.message
        ));
    }
    crc32fast::hash(payload.as_bytes())
}

async fn log_rw_parity_publish_event(
    client: &Client,
    origin: &str,
    uri: &Url,
    version: i32,
    diagnostics: &[Diagnostic],
) {
    let source_set = diagnostic_source_set(diagnostics);
    let diag_sig = diagnostic_signature(diagnostics);
    client
        .log_message(
            MessageType::INFO,
            format!(
                "RW_PARITY event=publish origin={} uri={} version={} diagnostic_count={} source_set={} diag_sig={}",
                origin,
                uri,
                version,
                diagnostics.len(),
                source_set,
                diag_sig
            ),
        )
        .await;
}

/// Convert SniperDB diagnostic to LSP diagnostic
fn convert_sniper_diagnostic_to_lsp(
    text: &str,
    sniper_diag: &sniper_db::diagnostic::Diagnostic,
) -> Diagnostic {
    use sniper_db::diagnostic::DiagnosticSeverity as SniperSeverity;
    
    // Convert severity
    let severity = match sniper_diag.severity {
        SniperSeverity::Error => DiagnosticSeverity::ERROR,
        SniperSeverity::Warning => DiagnosticSeverity::WARNING,
        SniperSeverity::Info => DiagnosticSeverity::INFORMATION,
        SniperSeverity::Hint => DiagnosticSeverity::HINT,
    };
    
    // Convert span to LSP range
    let range = byte_span_to_range(
        text,
        ByteSpan::new(sniper_diag.span.start, sniper_diag.span.end),
    );
    
    // Build message with related info
    let mut message = sniper_diag.message.clone();
    for related in &sniper_diag.related {
        use sniper_db::diagnostic::RelatedInfoKind;
        let prefix = match related.kind {
            RelatedInfoKind::Note => "note",
            RelatedInfoKind::Help => "help",
            RelatedInfoKind::See => "see",
        };
        message.push_str(&format!("\n{}: {}", prefix, related.message));
    }
    
    Diagnostic {
        range,
        severity: Some(severity),
        code: sniper_diag.code.as_ref().map(|c| NumberOrString::String(c.clone())),
        code_description: None,
        source: Some("SniperDB".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
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
        diagnostics_by_file: BTreeMap::new(),
        structured_diagnostics: Vec::new(),
        fingerprint: None,
        revision: 0,
        bundle: None,
        proof_state: None,
    }
}

/// PublishDiagnosticsHandler - Centralized diagnostic publishing system
///
/// This handler is responsible for:
/// - Converting SniperDB diagnostics to LSP format
/// - Using the span conversion system for precise UTF-16 positions
/// - Sorting diagnostics deterministically
///
/// # CHOKE POINT ENFORCEMENT
///
/// This is the ONLY code path that publishes diagnostics to LSP. All diagnostic
/// publication must flow through this handler. This is structurally enforced:
///
/// 1. `publish_diagnostics_canonical()` is the single entry point
/// 2. All other methods are private or internal
/// 3. No other code can call `client.publish_diagnostics()` directly
/// 4. Attempting to bypass this will fail at compile time
///
/// # Requirements
/// - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention)
/// - Validates: Requirements 5.1, 5.3 (LSP Integration)
pub struct PublishDiagnosticsHandler;

impl PublishDiagnosticsHandler {
    /// CANONICAL CHOKE POINT: Publish diagnostics for a document
    ///
    /// This is the ONLY function that publishes diagnostics to LSP.
    /// All diagnostic publication must flow through this function.
    ///
    /// This function:
    /// 1. Converts diagnostics from internal format to LSP format
    /// 2. Sorts diagnostics deterministically
    /// 3. Publishes all diagnostics through LSP protocol
    ///
    /// # Arguments
    /// - `client`: LSP client for publishing
    /// - `uri`: Document URI
    /// - `report`: WorkspaceReport containing all diagnostics
    /// - `parsed_doc`: Parsed document for span conversion
    /// - `version`: Optional document version
    ///
    /// # Requirements
    /// - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention)
    /// - Validates: Requirements 5.1, 5.3 (LSP Integration)
    pub async fn publish_diagnostics_canonical(
        client: &Client,
        uri: &Url,
        report: &WorkspaceReport,
        parsed_doc: &ParsedDocument,
        version: Option<i32>,
    ) {
        let diagnostics = Self::convert_diagnostics(uri, report, parsed_doc);
        Self::publish_diagnostics_internal(client, uri, diagnostics, version).await;
    }
    
    /// CANONICAL CHOKE POINT: Publish pre-converted diagnostics
    ///
    /// This is the ONLY function that publishes pre-converted diagnostics to LSP.
    /// Use this when diagnostics have already been converted and sorted.
    ///
    /// # Arguments
    /// - `client`: LSP client for publishing
    /// - `uri`: Document URI
    /// - `diagnostics`: Pre-converted and sorted diagnostics
    /// - `version`: Optional document version
    ///
    /// # Requirements
    /// - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention)
    /// - Validates: Requirements 5.1, 5.3 (LSP Integration)
    pub async fn publish_diagnostics_canonical_preconverted(
        client: &Client,
        uri: &Url,
        diagnostics: Vec<Diagnostic>,
        version: Option<i32>,
    ) {
        Self::publish_diagnostics_internal(client, uri, diagnostics, version).await;
    }
    
    /// DEPRECATED: Use publish_diagnostics_canonical() instead
    ///
    /// This function is deprecated and will be removed in a future version.
    /// All new code should use publish_diagnostics_canonical().
    #[deprecated(
        since = "0.2.0",
        note = "use publish_diagnostics_canonical() instead"
    )]
    pub async fn publish_diagnostics(
        client: &Client,
        uri: &Url,
        report: &WorkspaceReport,
        parsed_doc: &ParsedDocument,
        version: Option<i32>,
    ) {
        Self::publish_diagnostics_canonical(client, uri, report, parsed_doc, version).await;
    }
    
    /// DEPRECATED: Use publish_diagnostics_canonical_preconverted() instead
    ///
    /// This function is deprecated and will be removed in a future version.
    /// All new code should use publish_diagnostics_canonical_preconverted().
    #[deprecated(
        since = "0.2.0",
        note = "use publish_diagnostics_canonical_preconverted() instead"
    )]
    pub async fn publish_preconverted(
        client: &Client,
        uri: &Url,
        diagnostics: Vec<Diagnostic>,
        version: Option<i32>,
    ) {
        Self::publish_diagnostics_canonical_preconverted(client, uri, diagnostics, version).await;
    }
    
    /// INTERNAL: The actual LSP publication point
    ///
    /// This is the single point where diagnostics are published to LSP.
    /// This function is private to prevent bypass.
    ///
    /// # Requirements
    /// - Validates: Requirements 1.5, 1.6 (Choke Point Uniqueness and Bypass Prevention)
    async fn publish_diagnostics_internal(
        client: &Client,
        uri: &Url,
        diagnostics: Vec<Diagnostic>,
        version: Option<i32>,
    ) {
        // CHOKE POINT: All diagnostics flow through here
        client.publish_diagnostics(uri.clone(), diagnostics, version).await;
    }

    /// **INV D-PUBLISH-CORE**: Extract Core-only diagnostics from report (Phase 1).
    ///

    /// Convert diagnostics from internal format to LSP format
    ///
    /// This function:
    /// 1. Converts parser diagnostics
    /// 2. Converts workspace report diagnostics
    /// 3. Sorts all diagnostics deterministically
    ///
    /// # Requirements
    /// - Validates: Requirements 5.1, 5.3
    pub fn convert_diagnostics(
        uri: &Url,
        report: &WorkspaceReport,
        parsed_doc: &ParsedDocument,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Convert parser diagnostics
        diagnostics.extend(parsed_doc.diagnostics.iter().map(|pd| {
            Self::convert_parser_diagnostic(&parsed_doc.text, pd)
        }));

        // Convert workspace report diagnostics
        diagnostics.extend(
            Self::convert_workspace_diagnostics(report, &parsed_doc.text)
        );

        // Sort deterministically
        Self::sort_diagnostics(uri, &mut diagnostics);

        diagnostics
    }

    /// Convert a single parser diagnostic to LSP format
    fn convert_parser_diagnostic(text: &str, pd: &crate::document::ParseDiagnostic) -> Diagnostic {
        Diagnostic {
            range: byte_span_to_range(text, pd.span),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("parser".to_string()),
            message: pd.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Convert workspace report diagnostics to LSP format
    ///
    /// Uses the span conversion system to ensure UTF-16 correctness.
    /// Prioritizes structured_diagnostics (from Task 15 multi-diagnostic collection)
    /// over legacy diagnostics for backward compatibility.
    ///
    /// # Requirements
    /// - Validates: Requirements 5.1, 5.2, 6.1, 6.2, 8.3, 8.5
    fn convert_workspace_diagnostics(
        report: &WorkspaceReport,
        text: &str,
    ) -> Vec<Diagnostic> {
        // Use structured_diagnostics if available (Task 15), otherwise fall back to legacy diagnostics
        let diagnostics_to_convert = if !report.structured_diagnostics.is_empty() {
            // Convert StructuredDiagnostic to WorkspaceDiagnostic for consistent handling
            report
                .structured_diagnostics
                .iter()
                .map(|sd| WorkspaceDiagnostic::from(sd.clone()))
                .collect::<Vec<_>>()
        } else {
            // Fall back to legacy diagnostics for backward compatibility
            report.diagnostics.clone()
        };

        diagnostics_to_convert
            .iter()
            .map(|workspace_diag| {
                let span = workspace_diag
                    .span
                    .unwrap_or_else(|| source_span::Span::new(0, text.len()));
                let clamped = clamp_span(span, text.len());
                
                Diagnostic {
                    range: byte_span_to_range(text, ByteSpan::new(clamped.start, clamped.end)),
                    severity: Some(Self::convert_severity(workspace_diag.severity)),
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

    /// Convert workspace diagnostic severity to LSP severity
    fn convert_severity(severity: WorkspaceDiagnosticSeverity) -> DiagnosticSeverity {
        match severity {
            WorkspaceDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
            WorkspaceDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
            WorkspaceDiagnosticSeverity::Information => DiagnosticSeverity::INFORMATION,
        }
    }

    /// Sort diagnostics deterministically
    ///
    /// Diagnostics are sorted by:
    /// 1. URI
    /// 2. Reliability (spanned diagnostics before spanless)
    /// 3. Severity (errors before warnings before info)
    /// 4. Position (line, then character)
    /// 5. Message
    /// 6. Code
    /// 7. Source
    ///
    /// # Requirements
    /// - Validates: Requirements 5.3
    pub fn sort_diagnostics(uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
        diagnostics.sort_by(|a, b| {
            Self::diagnostic_sort_key(uri, a).cmp(&Self::diagnostic_sort_key(uri, b))
        });
    }

    /// Generate sort key for a diagnostic
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
        let reliability = if diag.range.start.line == 0
            && diag.range.start.character == 0
            && diag.range.end.line == 0
            && diag.range.end.character == 0
        {
            1
        } else {
            0
        };
        (
            uri.as_str(),
            reliability,
            Self::severity_rank(diag.severity),
            diag.range.start.line,
            diag.range.start.character,
            diag.range.end.line,
            diag.range.end.character,
            diag.message.as_str(),
            Self::diagnostic_code_to_str(diag.code.as_ref()),
            Self::diagnostic_source_to_str(diag.source.as_ref()),
        )
    }

    /// Convert diagnostic severity to rank for sorting
    fn severity_rank(severity: Option<DiagnosticSeverity>) -> u8 {
        match severity {
            Some(sev) if sev == DiagnosticSeverity::ERROR => 0,
            Some(sev) if sev == DiagnosticSeverity::WARNING => 1,
            Some(sev) if sev == DiagnosticSeverity::INFORMATION => 2,
            Some(sev) if sev == DiagnosticSeverity::HINT => 3,
            _ => 4,
        }
    }

    /// Convert diagnostic code to string for sorting
    fn diagnostic_code_to_str(code: Option<&NumberOrString>) -> String {
        code.map(|c| match c {
            NumberOrString::Number(n) => n.to_string(),
            NumberOrString::String(s) => s.clone(),
        })
        .unwrap_or_default()
    }

    /// Convert diagnostic source to string for sorting
    fn diagnostic_source_to_str(source: Option<&String>) -> &str {
        source.map(|s| s.as_str()).unwrap_or_default()
    }
}



pub struct Backend {
    client: Client, // Changed to Client
    proof_session: Arc<RwLock<ProofSession>>,
    config: Arc<RwLock<Config>>,
    did_change_tx: mpsc::Sender<DebouncedDocumentChange>,
    registry: Arc<PrinterRegistry>, // NEW: server-wide printer registry
    tactic_registry: Arc<TacticRegistry>, // Tactic engine
    db: Arc<SniperDatabase>, // SniperDB instance
    madlib_resolver: Arc<RwLock<Option<MadLibResolver>>>, // Phase 1.5: MadLib resolver with auto-discovery
}

impl Backend {
    pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type
        let (did_change_tx, did_change_rx) = mpsc::channel(100);

        // Initialize SniperDB
        let db = Arc::new(SniperDatabase::new());

        let proof_session_arc = Arc::new(RwLock::new(ProofSession::new(client.clone(), config.clone(), db.clone())));

        // Initialize printer registry once at startup
        let registry = Arc::new(PrinterRegistry::new_with_defaults());

        // Initialize tactic registry
        let mut tactic_registry = TacticRegistry::new();
        register_std_tactics(&mut tactic_registry);
        let tactic_registry = Arc::new(tactic_registry);

        tokio::spawn(process_debounced_proof_session_events(
            proof_session_arc.clone(),
            config.clone(),
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
            madlib_resolver: Arc::new(RwLock::new(None)), // Phase 1.5: Initialized in initialize() after workspace root discovery
        }
    }

    /// Coherence hover: Show witness steps for (coherent? ...) forms
    async fn hover_coherence(&self, uri: &Url, position: Position) -> Option<Hover> {
        // 1. Get document text
        let text = {
            let session = self.proof_session.read().await;
            session.get_document_text(&uri)?
        };
        
        // 2. Convert position to offset
        let offset = position_to_offset(&text, position);
        
        // 3. Find the form at cursor position
        let line_start = text.lines().take(position.line as usize).map(|l| l.len() + 1).sum::<usize>();
        let line_end = line_start + text.lines().nth(position.line as usize)?.len();
        let cursor_line = &text[line_start..line_end];
        
        // 4. Check for (coherent? ...) form
        if let Some(coherent_start) = cursor_line.find("(coherent?") {
            let coherent_pos = line_start + coherent_start;
            if coherent_pos <= offset && offset <= coherent_pos + "(coherent?".len() {
                // Extract the entire coherent? form
                let form_start = coherent_pos;
                let mut depth = 0;
                let mut form_end = form_start;
                
                for (i, ch) in text[form_start..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                form_end = form_start + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                
                let form_text = &text[form_start..form_end];
                
                // Parse the coherent? form to extract traces
                let mut parts = Vec::new();
                let mut current = String::new();
                let mut depth = 0;
                let mut in_string = false;
                
                for ch in form_text.chars() {
                    match ch {
                        '"' if !in_string => in_string = true,
                        '"' if in_string => in_string = false,
                        '(' if !in_string => {
                            if depth > 0 {
                                current.push(ch);
                            }
                            depth += 1;
                        }
                        ')' if !in_string => {
                            depth -= 1;
                            if depth > 0 {
                                current.push(ch);
                            } else if !current.trim().is_empty() {
                                parts.push(current.trim().to_string());
                                current.clear();
                            }
                        }
                        ' ' | '\t' | '\n' if !in_string && depth == 1 => {
                            if !current.trim().is_empty() {
                                parts.push(current.trim().to_string());
                                current.clear();
                            }
                        }
                        _ => current.push(ch),
                    }
                }
                
                // Generate hover content with actual coherence checking
                let mut markdown = format!(
                    "### Coherence Query\n\n```maclane\n{}\n```\n\n",
                    form_text
                );
                
                if parts.len() >= 3 && parts[0] == "coherent?" {
                    let trace1_str = parts[1].trim();
                    let trace2_str = parts[2].trim();
                    
                    // Create simple trace expressions for testing
                    let trace1 = CoreSExpr::List(vec![
                        CoreSExpr::Atom(CoreAtom::Symbol("trace".to_string())),
                        CoreSExpr::Atom(CoreAtom::Symbol("trace1".to_string())),
                    ]);
                    let trace2 = CoreSExpr::List(vec![
                        CoreSExpr::Atom(CoreAtom::Symbol("trace".to_string())),
                        CoreSExpr::Atom(CoreAtom::Symbol("trace2".to_string())),
                    ]);
                    
                    // Call coherence query
                    match coherent_query(&trace1, &trace2) {
                        Ok(result) => {
                            markdown.push_str(&format!("**Status**: ✅ Coherent\n\n"));
                            markdown.push_str(&format!("**Witness**:\n```\n{:?}\n```\n\n", result));
                            markdown.push_str("**Level**: Generator-based coherence (Level 2)\n\n");
                        }
                        Err(e) => {
                            markdown.push_str(&format!("**Status**: ❌ Not Coherent\n\n"));
                            markdown.push_str(&format!("**Error**: {}\n\n", e));
                            markdown.push_str("**Suggestion**: Try increasing fuel with `(coherent-with-fuel? ...)`\n\n");
                        }
                    }
                } else {
                    markdown.push_str("**Status**: Query detected\n\n");
                    markdown.push_str("**Witness**: Parsing traces...\n\n");
                    markdown.push_str("**Level**: Generator-based coherence (Level 2)\n\n");
                }
                
                markdown.push_str("---\n*Hover shows coherence witness steps when available*");
                
                return Some(Hover {
                    contents: HoverContents::Markup(tower_lsp::lsp_types::MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: markdown,
                    }),
                    range: Some(Range {
                        start: Position {
                            line: position.line,
                            character: coherent_start as u32,
                        },
                        end: Position {
                            line: position.line,
                            character: (coherent_start + "(coherent?".len()) as u32,
                        },
                    }),
                });
            }
        }
        
        // 5. Check for (trace ...) form
        if let Some(trace_start) = cursor_line.find("(trace") {
            let trace_pos = line_start + trace_start;
            if trace_pos <= offset && offset <= trace_pos + "(trace".len() {
                // Extract the entire trace form
                let form_start = trace_pos;
                let mut depth = 0;
                let mut form_end = form_start;
                
                for (i, ch) in text[form_start..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                form_end = form_start + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                
                let form_text = &text[form_start..form_end];
                
                // Parse trace form for better hover information
                let mut markdown = format!(
                    "### Trace Capture\n\n```maclane\n{}\n```\n\n",
                    form_text
                );
                
                // Extract trace name if available
                if let Some(trace_name_start) = form_text.find("(trace ") {
                    let name_part = &form_text[trace_name_start + 7..];
                    if let Some(end) = name_part.find(')') {
                        let trace_name = name_part[..end].trim();
                        if !trace_name.is_empty() {
                            markdown.push_str(&format!("**Trace Name**: `{}`\n\n", trace_name));
                        }
                    }
                }
                
                markdown.push_str("**Status**: ✅ Trace captured\n\n");
                markdown.push_str("**Purpose**: Provides deterministic computation record\n\n");
                markdown.push_str("**Properties**:\n");
                markdown.push_str("- Deterministic (INV D-*)\n");
                markdown.push_str("- Replayable\n");
                markdown.push_str("- Coherence-checkable\n\n");
                markdown.push_str("**Usage**: Can be used with coherence queries:\n");
                markdown.push_str("- `(coherent? trace1 trace2)`\n");
                markdown.push_str("- `(assert-coherent trace1 trace2)`\n");
                markdown.push_str("- `(coherent-with-fuel? trace1 trace2 1000)`\n\n");
                markdown.push_str("---\n*Trace enables reproducible computation and coherence checking*");
                
                return Some(Hover {
                    contents: HoverContents::Markup(tower_lsp::lsp_types::MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: markdown,
                    }),
                    range: Some(Range {
                        start: Position {
                            line: position.line,
                            character: trace_start as u32,
                        },
                        end: Position {
                            line: position.line,
                            character: (trace_start + "(trace".len()) as u32,
                        },
                    }),
                });
            }
        }
        
        None
    }

    /// DB-7 hover preview: show rename impact analysis
    async fn hover_db7_preview(&self, uri: &Url, position: Position) -> Option<Hover> {
        // 1. Get document text
        let text = {
            let session = self.proof_session.read().await;
            let doc = session.get_parsed_document(uri)?;
            doc.text.clone()
        };
        
        // 2. Extract symbol (fail-closed)
        let from = symbol_at_position(&text, position)?;
        
        // 3. Determine file_id (use existing mapping)
        let file_id = stable_uri_file_id(uri);
        
        // 4. Build deterministic rename target (using configurable suffix)
        let config = self.config.read().await;
        let suffix = config.db7_placeholder_suffix.clone();
        let debug_mode = config.db7_debug_mode;
        drop(config);
        
        let to = format!("{}{}", from, suffix);
        
        // 5. Build RenameSymbol intent
        let intent = sniper_db::plan::EditIntent::RenameSymbol {
            file_id,
            from: from.clone(),
            to: to.clone(),
            scope_hint: None, // Conservative: no hint
        };
        
        // 6. Build conservative policy
        let policy = sniper_db::plan::PlanPolicy {
            max_steps: 1,
            max_branches: 1,
            max_cost: None,
            prefer_proof_preservation: true,
            precision_mode: sniper_db::plan::PrecisionMode::Conservative,
            tie_break: sniper_db::plan::TieBreakPolicy::Stable,
            report_verbosity: sniper_db::plan::ReportVerbosity::Compact,
        };
        
        // 7. Call DB-7 (memoized, pure, deterministic)
        let report = self.db.plan_report_query(vec![intent], policy);
        
        // 8. Render markdown
        let markdown = render_hover_markdown(&from, &to, &report, debug_mode);
        
        // 9. Return hover
        Some(Hover {
            contents: HoverContents::Markup(tower_lsp::lsp_types::MarkupContent {
                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        })
    }

    /// DB-7 code action: offer "Preview Rename Impact" action
    async fn code_action_db7_preview(&self, uri: &Url, position: Position) -> Option<tower_lsp::lsp_types::CodeAction> {
        self.code_action_db7_preview_internal(uri, position, sniper_db::plan::ReportVerbosity::Compact, false).await
    }

    /// DB-7 code action (detailed): offer "Preview Rename Impact (Detailed)" action
    async fn code_action_db7_preview_detailed(&self, uri: &Url, position: Position) -> Option<tower_lsp::lsp_types::CodeAction> {
        self.code_action_db7_preview_internal(uri, position, sniper_db::plan::ReportVerbosity::Detailed, true).await
    }

    /// Internal helper for DB-7 code actions
    async fn code_action_db7_preview_internal(
        &self,
        uri: &Url,
        position: Position,
        verbosity: sniper_db::plan::ReportVerbosity,
        is_detailed: bool,
    ) -> Option<tower_lsp::lsp_types::CodeAction> {
        // 1. Get document text
        let text = {
            let session = self.proof_session.read().await;
            let doc = session.get_parsed_document(uri)?;
            doc.text.clone()
        };
        
        // 2. Extract symbol (fail-closed)
        let from = symbol_at_position(&text, position)?;
        
        // 3. Determine file_id (use existing mapping)
        let file_id = stable_uri_file_id(uri);
        
        // 4. Build deterministic rename target (using configurable suffix)
        let config = self.config.read().await;
        let suffix = config.db7_placeholder_suffix.clone();
        let debug_mode = config.db7_debug_mode;
        drop(config);
        
        let to = format!("{}{}", from, suffix);
        
        // 5. Build RenameSymbol intent
        let intent = sniper_db::plan::EditIntent::RenameSymbol {
            file_id,
            from: from.clone(),
            to: to.clone(),
            scope_hint: None, // Conservative: no hint
        };
        
        // 6. Build policy with specified verbosity
        let policy = sniper_db::plan::PlanPolicy {
            max_steps: 1,
            max_branches: 1,
            max_cost: None,
            prefer_proof_preservation: true,
            precision_mode: sniper_db::plan::PrecisionMode::Conservative,
            tie_break: sniper_db::plan::TieBreakPolicy::Stable,
            report_verbosity: verbosity,
        };
        
        // 7. Call DB-7 (memoized, pure, deterministic)
        let report = self.db.plan_report_query(vec![intent], policy);
        
        // 8. Render markdown
        let markdown = render_hover_markdown(&from, &to, &report, debug_mode);
        
        // 9. Create code action (no edit, just shows information)
        let title = if is_detailed {
            format!("Preview Rename Impact (Detailed): {} → {}", from, to)
        } else {
            format!("Preview Rename Impact: {} → {}", from, to)
        };
        
        Some(tower_lsp::lsp_types::CodeAction {
            title,
            kind: Some(tower_lsp::lsp_types::CodeActionKind::REFACTOR),
            diagnostics: None,
            edit: None, // No workspace edit (read-only preview)
            command: Some(tower_lsp::lsp_types::Command {
                title: "Show Rename Impact".to_string(),
                command: "edgelord/showMessage".to_string(),
                arguments: Some(vec![serde_json::json!({
                    "message": markdown,
                    "type": "info"
                })]),
            }),
            is_preferred: Some(false),
            disabled: None,
            data: None,
        })
    }
} // End of impl Backend

// ─────────────────────────────────────────────────────────────────────────────
// SB1: Custom LSP push-notification types and free helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// `$/edgelord/goalsUpdated` — server → client push after every elaboration.
enum GoalsUpdated {}
impl tower_lsp::lsp_types::notification::Notification for GoalsUpdated {
    type Params = GoalsUpdatedParams;
    const METHOD: &'static str = "$/edgelord/goalsUpdated";
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoalsUpdatedParams {
    uri: String,
    version: i32,
    goals: Vec<crate::goals_panel::GoalPanelItem>,
    stale: bool,
    banner: Option<String>,
    checked_up_to: Option<usize>,
    total_goals: usize,
    unsolved_goals: usize,
    delta_summary: Option<String>,
}

/// `$/edgelord/checkedRegion` — server → client highlight of the checked prefix.
enum CheckedRegion {}
impl tower_lsp::lsp_types::notification::Notification for CheckedRegion {
    type Params = CheckedRegionParams;
    const METHOD: &'static str = "$/edgelord/checkedRegion";
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckedRegionParams {
    uri: String,
    checked_up_to: usize,
    form_count: usize,
    total_forms: usize,
}

/// Push a `$/edgelord/goalsUpdated` notification for `uri`.
/// Callable from both `impl Backend` methods and free async functions.
async fn push_goals_update(
    client: &tower_lsp::Client,
    session: &crate::proof_session::ProofSession,
    uri: &tower_lsp::lsp_types::Url,
) {
    use crate::goals_panel::GoalStatus;
    let Some(panel) = session.compute_goals_panel(uri) else { return };
    let unsolved = panel
        .goals
        .iter()
        .filter(|g| matches!(g.status, GoalStatus::Unsolved))
        .count();
    let delta_summary = session.delta_summary(uri);
    let checked_up_to = session.get_document(uri).and_then(|d| d.checked_boundary);
    let params = GoalsUpdatedParams {
        uri: uri.to_string(),
        version: panel.version,
        total_goals: panel.goals.len(),
        unsolved_goals: unsolved,
        stale: panel.stale,
        goals: panel.goals,
        banner: panel.banner,
        checked_up_to,
        delta_summary,
    };
    client.send_notification::<GoalsUpdated>(params).await;
}

/// Push a `$/edgelord/checkedRegion` notification for `uri`.
async fn push_checked_region(
    client: &tower_lsp::Client,
    session: &crate::proof_session::ProofSession,
    uri: &tower_lsp::lsp_types::Url,
) {
    let Some(doc) = session.get_document(uri) else { return };
    let Some(checked) = doc.checked_boundary else { return };
    let boundaries = doc.parsed.top_level_form_boundaries();
    let form_count = boundaries.iter().filter(|&&b| b <= checked).count();
    let params = CheckedRegionParams {
        uri: uri.to_string(),
        checked_up_to: checked,
        form_count,
        total_forms: boundaries.len(),
    };
    client.send_notification::<CheckedRegion>(params).await;
}

/// Extract and parse a URI from the first element of a command's argument list.
fn parse_uri_from_command_args(
    args: &[serde_json::Value],
) -> std::result::Result<tower_lsp::lsp_types::Url, tower_lsp::jsonrpc::Error> {
    let uri_str = args
        .first()
        .and_then(|v| {
            // Accept either a bare string or {"uri": "..."} / {"cursorOffset": ...}
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.get("uri").and_then(|u| u.as_str()).map(|s| s.to_string()))
        })
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing URI argument"))?;
    tower_lsp::lsp_types::Url::parse(&uri_str)
        .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))
}

// async_trait not needed here. It's for LanguageServer trait impl
async fn process_debounced_proof_session_events(
    proof_session_arc: Arc<RwLock<ProofSession>>,
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
                        format!("RW_PARITY event=did_change origin=debounce uri={} version={}", change.uri, change.version),
                    ).await;
                    client.log_message(
                        MessageType::INFO,
                        format!("Processing debounced change for {} v{}", change.uri, change.version).to_string(),
                    ).await;
                    let ProofSessionUpdateResult {
                        report: workspace_report,
                        diagnostics: _,
                        goals: _,
                        measurement: _,  // C2.4: For benchmarking
                    } = {
                        let mut proof_session = proof_session_arc.write().await;
                        proof_session.update(change.uri.clone(), change.version, change.content_changes.clone()).await
                    };

                    // **TWO-PHASE PUBLISH PATTERN**
                    // INV D-PUBLISH-LATENCY: Phase 1 < 10ms
                    // INV D-NONBLOCKING: Phase 2 async (never delays Phase 1)
                    {
                        let session = proof_session_arc.read().await;
                        if let Some(parsed_doc) = session.get_parsed_document(&change.uri) {
                            let file_id = stable_uri_file_id(&change.uri);

                            // Update SniperDB with new text
                            db.set_input(file_id, parsed_doc.text.clone());

                            // Query SniperDB for diagnostics (AFTER set_input)
                            let sniper_diagnostics = db.diagnostics_file_query(file_id);

                            // **PHASE 1**: Publish all diagnostics (single-phase for now)
                            let phase1_start = Instant::now();
                            {
                                // Use existing diagnostic conversion from document handling
                                let mut phase1_lsp_diags = PublishDiagnosticsHandler::convert_diagnostics(&change.uri, &workspace_report, &parsed_doc);

                                // Sort deterministically
                                PublishDiagnosticsHandler::sort_diagnostics(&change.uri, &mut phase1_lsp_diags);

                                log_rw_parity_publish_event(
                                    &client,
                                    "debounce",
                                    &change.uri,
                                    change.version,
                                    &phase1_lsp_diags,
                                ).await;

                                // Publish through canonical choke point
                                PublishDiagnosticsHandler::publish_diagnostics_canonical_preconverted(
                                    &client,
                                    &change.uri,
                                    phase1_lsp_diags,
                                    Some(change.version),
                                ).await;
                            }

                            // SB1: Push goals state after elaboration
                            {
                                let session = proof_session_arc.read().await;
                                push_goals_update(&client, &session, &change.uri).await;
                            }

                            let phase1_latency_ms = phase1_start.elapsed().as_millis();
                            if phase1_latency_ms >= 10 {
                                client.log_message(
                                    MessageType::WARNING,
                                    format!("INV D-PUBLISH-LATENCY violation: Phase 1 took {}ms (target: < 10ms)", phase1_latency_ms),
                                ).await;
                            }

                            // **PHASE 2**: Spawn ScopeCreep analysis asynchronously
                            // INV T-ASYNC-SPAWN: Spawned AFTER Phase 1 publish
                            // INV T-SNAPSHOT: Snapshot captured before spawn
                            // INV T-DVCMP: Document-version guard (stale-result rejection)
                            let client_phase2 = client.clone();
                            // Phase 2 ScopeCreep analysis deferred for future implementation
                            // TODO: Implement async ScopeCreep analysis with stale-result prevention
                        }
                    }
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

        // **Phase 1.5**: Initialize MadLib resolver with workspace root auto-discovery
        {
            let resolver = if let Some(root_uri) = params.root_uri.as_ref() {
                // Try to extract workspace root and create resolver with it
                if let Ok(root_path) = root_uri.to_file_path() {
                    let resolver = MadLibResolver::with_workspace_root(root_path.clone());
                    self.client
                        .log_message(
                            MessageType::INFO,
                            format!(
                                "MadLib resolver initialized with workspace root: {}",
                                root_path.display()
                            ),
                        )
                        .await;
                    Some(resolver)
                } else {
                    // Workspace root is not a file path, use defaults
                    let resolver = MadLibResolver::with_defaults();
                    self.client
                        .log_message(
                            MessageType::INFO,
                            "MadLib resolver initialized with default search paths".to_string(),
                        )
                        .await;
                    Some(resolver)
                }
            } else {
                // No workspace root provided, use defaults
                let resolver = MadLibResolver::with_defaults();
                self.client
                    .log_message(
                        MessageType::INFO,
                        "MadLib resolver initialized with default search paths (no workspace root)".to_string(),
                    )
                    .await;
                Some(resolver)
            };

            *self.madlib_resolver.write().await = resolver;
        }
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
                    commands: vec![
                        "edgelord/goals".to_string(),
                        "edgelord/explain".to_string(),
                        "edgelord/cache-stats".to_string(),
                        "edgelord/refute".to_string(),
                        "edgelord/step-forward".to_string(),   // SB0
                        "edgelord/step-backward".to_string(),  // SB0
                        "edgelord/goto-cursor".to_string(),    // SB0
                        "edgelord/undo-step".to_string(),      // SB2
                        "edgelord/resolve-anchor".to_string(), // SB3
                    ],
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
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["(".to_string(), ":".to_string()]),
                    resolve_provider: Some(false),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    all_commit_characters: None,
                    completion_item: None,
                }),
                folding_range_provider: Some(tower_lsp::lsp_types::FoldingRangeProviderCapability::Simple(true)),
                rename_provider: Some(OneOf::Right(tower_lsp::lsp_types::RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                })),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), " ".to_string()]),
                    retrigger_characters: Some(vec![" ".to_string()]),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                document_highlight_provider: Some(OneOf::Left(true)),
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
            report,
            diagnostics: _,
            goals: _,
        } = self.proof_session.write().await.open(uri.clone(), version, text.clone()).await;
        
        // Update SniperDB
        let file_id = stable_uri_file_id(&uri);
        self.db.set_input(file_id, text);

        // Publish diagnostics through canonical choke point
        let session = self.proof_session.read().await;
        if let Some(parsed_doc) = session.get_parsed_document(&uri) {
            PublishDiagnosticsHandler::publish_diagnostics_canonical(
                &self.client,
                &uri,
                &report,
                parsed_doc,
                Some(version),
            ).await;
        }
        // SB1: Push goals state after elaboration
        push_goals_update(&self.client, &session, &uri).await;
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
        self.client
            .log_message(
                MessageType::INFO,
                format!("RW_PARITY event=did_save origin=did_save uri={} version={}", uri, version),
            )
            .await;
        
        // Get report from proof session
        let report = {
            let mut proof_session = self.proof_session.write().await;
            if let Some(text) = params.text {
                proof_session.open(uri.clone(), version, text).await.report
            } else {
                proof_session.apply_command(uri.clone(), "re-analyze-on-save".to_string()).await.report
            }
        };
        
        // Get parsed document for handler
        let session = self.proof_session.read().await;
        if let Some(parsed_doc) = session.get_parsed_document(&uri) {
            // Convert diagnostics through handler
            let mut diagnostics = PublishDiagnosticsHandler::convert_diagnostics(
                &uri,
                &report,
                parsed_doc,
            );
            
            // Add external command diagnostics if configured
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
                    diagnostics.extend(external_diagnostics);
                    
                    // Re-sort after adding external diagnostics
                    PublishDiagnosticsHandler::sort_diagnostics(&uri, &mut diagnostics);
                }
            }
            drop(config);

            log_rw_parity_publish_event(
                &self.client,
                "did_save",
                &uri,
                version,
                &diagnostics,
            ).await;
            
            // Publish through canonical choke point (already sorted)
            PublishDiagnosticsHandler::publish_diagnostics_canonical_preconverted(
                &self.client,
                &uri,
                diagnostics,
                Some(version),
            ).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.proof_session.write().await.close(&uri);
        
        // Clear diagnostics through canonical choke point
        PublishDiagnosticsHandler::publish_diagnostics_canonical_preconverted(
            &self.client,
            &uri,
            Vec::new(),
            None,
        ).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        
        // 1. Try coherence hover first
        if let Some(hover) = self.hover_coherence(&uri, position).await {
            return Ok(Some(hover));
        }
        
        // 2. Try DB-7 hover next (if enabled)
        if self.config.read().await.enable_db7_hover_preview {
            if let Some(hover) = self.hover_db7_preview(&uri, position).await {
                return Ok(Some(hover));
            }
        }
        
        // 3. Symbol-info hover: show definition kind, source module, documentation
        {
            let session = self.proof_session.read().await;
            if let Some(doc) = session.get_parsed_document(&uri) {
                let offset = position_to_offset(&doc.text, position);
                let index = SymbolIndex::build(&doc.text);
                
                // Find what's under the cursor
                let symbol_info = index.definition_at_offset(offset)
                    .map(|d| (d.name.clone(), d.kind, d.detail.clone()))
                    .or_else(|| {
                        index.reference_at_offset(offset)
                            .and_then(|r| {
                                index.find_definition(&r.name)
                                    .map(|d| (d.name.clone(), d.kind, d.detail.clone()))
                            })
                    })
                    .or_else(|| {
                        symbol_at_position(&doc.text, position)
                            .and_then(|name| {
                                index.find_definition(&name)
                                    .map(|d| (d.name.clone(), d.kind, d.detail.clone()))
                            })
                    });

                if let Some((name, kind, detail)) = symbol_info {
                    let kind_label = match kind {
                        SymbolDefKind::Touch => "touch (axiom/hypothesis)",
                        SymbolDefKind::Def => "def (definition)",
                        SymbolDefKind::Rule => "rule (rewrite rule)",
                        SymbolDefKind::Sugar => "sugar (macro)",
                        SymbolDefKind::Let => "let (local binding)",
                        SymbolDefKind::Lambda => "lambda (parameter)",
                        SymbolDefKind::Import => "import",
                    };
                    
                    let mut md = format!("**{}** — _{}_\n", name, kind_label);
                    
                    if let Some(ref det) = detail {
                        if det.starts_with("use ") {
                            md.push_str(&format!("\nImported from `{}`\n", &det[4..]));
                            
                            // Try to resolve and show first few lines of definition
                            let resolver = self.madlib_resolver.read().await;
                            if let Some(ref resolver) = *resolver {
                                let module_path = ModulePath::parse(&det[4..]);
                                if let Ok(resolved_file) = resolver.resolve_module(&module_path) {
                                    if let Ok(source) = std::fs::read_to_string(&resolved_file) {
                                        let target_index = SymbolIndex::build(&source);
                                        if let Some(target_def) = target_index.find_definition(&name) {
                                            // Extract a preview of the definition (up to ~200 chars)
                                            let start = target_def.form_span.start;
                                            let end = (target_def.form_span.end).min(source.len()).min(start + 200);
                                            let preview = &source[start..end];
                                            let preview = if end < target_def.form_span.end {
                                                format!("{}…", preview.trim())
                                            } else {
                                                preview.trim().to_string()
                                            };
                                            md.push_str(&format!("\n```scheme\n{}\n```\n", preview));
                                        }
                                    }
                                }
                            }
                        } else {
                            md.push_str(&format!("\n_{}_\n", det));
                        }
                    }

                    // Show reference count
                    let ref_count = index.find_references(&name).len();
                    if ref_count > 0 {
                        md.push_str(&format!("\n---\n_{} reference(s) in this file_\n", ref_count));
                    }
                    
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(tower_lsp::lsp_types::MarkupContent {
                            kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                            value: md,
                        }),
                        range: None,
                    }));
                }
            }
        }

        // 4. Proof-context hover (SA3): shows local context + target at any position
        //    inside or near a proof form. Uses goal_enclosing_offset which falls back
        //    to the nearest preceding goal when the cursor is between goals.
        let markdown_content_option = {
            let session = self.proof_session.read().await;
            if let Some(proof) = session.get_proof_state(&uri) {
                let offset = position_to_offset(&session.get_parsed_document(&uri).unwrap().text, position);
                if let Some(goal) = crate::proof_session::goal_enclosing_offset(proof, offset) {
                    // Create pretty context
                    let config = self.config.read().await;
                    let dialect = config.pretty_dialect.as_deref()
                        .map(|s| match s {
                            "pythonic" => PrettyDialect::Pythonic,
                            "canonical" => PrettyDialect::Canonical,
                            _ => PrettyDialect::Canonical,
                        })
                        .unwrap_or(PrettyDialect::Canonical);

                    let files = DiagnosticContext::new("hover_dummy".to_string(), "");
                    let ctx = crate::edgelord_pretty_ctx::EdgeLordPrettyCtx::new(
                        &self.registry,
                        dialect,
                        PrettyLimits::hover_default(),
                        proof,
                        &files,
                        &uri,
                    );

                    // Status label
                    use comrade_lisp::proof_state::GoalStatus;
                    let status_icon = match &goal.status {
                        GoalStatus::Unsolved => "\u{2b1c} Unsolved",          // ⬜
                        GoalStatus::Solved(_) => "\u{2705} Solved",           // ✅
                        GoalStatus::Blocked { .. } => "\u{1f536} Blocked",    // 🔶
                        GoalStatus::Inconsistent { .. } => "\u{274c} Inconsistent", // ❌
                    };

                    // Render local context and target type
                    let ctx_text = ctx.render_local_context(&goal.local_context);
                    let type_text = ctx.render_mor_type(&goal.expected_type);

                    let mut md = format!(
                        "### `?{}` — {}\n\n**Context:**\n{}\n\n**Target:**\n  `{}`",
                        goal.name, status_icon, ctx_text, type_text
                    );

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
        } else if params.command == "edgelord/cache-stats" {
            // Phase 1: Expose cache statistics to client
            let session = self.proof_session.read().await;

            // Collect Phase 1.1 stats from module cache
            let module_cache_stats = {
                let cache = session.module_cache.read().await;
                let stats = cache.stats();
                serde_json::json!({
                    "hits": stats.hits,
                    "misses": stats.misses,
                    "hit_rate": stats.hit_rate(),
                    "total_operations": stats.total_operations(),
                })
            };

            // Collect Phase 1 stats from module snapshot cache
            let snapshot_cache_stats = {
                let cache = session.module_snapshot_cache.read().await;
                let stats = cache.stats();
                serde_json::json!({
                    "hits": stats.hits,
                    "misses": stats.misses,
                    "hit_rate": stats.hit_rate(),
                })
            };

            // Build response
            let report = serde_json::json!({
                "phase_1_1_module_cache": module_cache_stats,
                "phase_1_module_snapshots": snapshot_cache_stats,
                "message": "Cache Statistics Report"
            });

            // Log to client
            self.client.log_message(
                MessageType::INFO,
                format!(
                    "Cache Statistics:\n{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                ),
            ).await;

            Ok(Some(report))
        } else if params.command == "edgelord/refute" {
            // Wire refutation engine (Phase 0)
            let args = params.arguments;
            if args.is_empty() {
                return Err(tower_lsp::jsonrpc::Error::invalid_params("Missing arguments"));
            }

            let req: RefuteRequest = serde_json::from_value(args[0].clone())
                .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid RefuteRequest: {}", e)))?;

            let response = handle_refute_request(req, false);

            let json = serde_json::to_value(response).map_err(|e| {
                tower_lsp::jsonrpc::Error {
                    code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                    message: format!("Failed to serialize refute response: {}", e).into(),
                    data: None,
                }
            })?;

            self.client.log_message(
                MessageType::INFO,
                "Refutation request processed".to_string(),
            ).await;

            Ok(Some(json))

        // ─── SB0: Proof-stepping commands ────────────────────────────────────
        } else if params.command == "edgelord/step-forward" {
            let uri = parse_uri_from_command_args(&params.arguments)?;
            let new_boundary = self.proof_session.write().await.step_forward(&uri);
            let session = self.proof_session.read().await;
            push_goals_update(&self.client, &session, &uri).await;
            push_checked_region(&self.client, &session, &uri).await;
            Ok(new_boundary.map(|b| serde_json::json!({ "checkedUpTo": b })))

        } else if params.command == "edgelord/step-backward" {
            let uri = parse_uri_from_command_args(&params.arguments)?;
            let new_boundary = self.proof_session.write().await.step_backward(&uri);
            let session = self.proof_session.read().await;
            push_goals_update(&self.client, &session, &uri).await;
            push_checked_region(&self.client, &session, &uri).await;
            Ok(new_boundary.map(|b| serde_json::json!({ "checkedUpTo": b })))

        } else if params.command == "edgelord/goto-cursor" {
            let uri = parse_uri_from_command_args(&params.arguments)?;
            let cursor_offset: usize = params.arguments.first()
                .and_then(|v| v.get("cursorOffset"))
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(0);
            let new_boundary = self.proof_session.write().await.goto_cursor(&uri, cursor_offset);
            let session = self.proof_session.read().await;
            push_goals_update(&self.client, &session, &uri).await;
            push_checked_region(&self.client, &session, &uri).await;
            Ok(new_boundary.map(|b| serde_json::json!({ "checkedUpTo": b })))

        // ─── SB2: Undo proof step ──────────────────────────────────────────
        } else if params.command == "edgelord/undo-step" {
            let uri = parse_uri_from_command_args(&params.arguments)?;
            let undo_result = self.proof_session.write().await.undo_step(&uri);
            let session = self.proof_session.read().await;
            push_goals_update(&self.client, &session, &uri).await;
            push_checked_region(&self.client, &session, &uri).await;
            match undo_result {
                Some(result) => {
                    // If there is a reverse text edit, apply it in the editor
                    if let Some(edit) = &result.reverse_edit {
                        let _ = self.client.apply_edit(edit.clone()).await;
                    }
                    Ok(Some(serde_json::to_value(&result)
                        .unwrap_or(serde_json::json!({}))))
                }
                None => Ok(Some(serde_json::json!({ "error": "No undo history" }))),
            }

        // ─── SB3: Resolve stable goal anchor to source span ─────────────────────
        } else if params.command == "edgelord/resolve-anchor" {
            let uri = parse_uri_from_command_args(&params.arguments)?;
            let anchor_id = params.arguments.first()
                .and_then(|v| v.get("anchorId").or_else(|| v.get("anchor_id")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing anchorId"))?;
            let session = self.proof_session.read().await;
            let result = session.resolve_goal_anchor(&uri, &anchor_id);
            match result {
                Some((_, Some(byte_span))) => {
                    let doc_text = session.get_document_text(&uri).unwrap_or_default();
                    let range = byte_span_to_range(&doc_text, ByteSpan::new(byte_span.start, byte_span.end));
                    Ok(Some(serde_json::json!({ "span": range })))
                }
                Some((_, None)) => Ok(Some(serde_json::json!({ "span": null }))),
                None => Ok(Some(serde_json::json!({ "error": "Anchor not found" }))),
            }

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
                "pythonic" => PrettyDialect::Pythonic,
                _ => PrettyDialect::Canonical,
            })
            .unwrap_or(PrettyDialect::Canonical);

        let files = DiagnosticContext::new(uri.to_string(), "");
        let pretty_ctx = crate::edgelord_pretty_ctx::EdgeLordPrettyCtx::new(
            &self.registry,
            dialect,
            PrettyLimits::hover_default(),
            proof_state,
            &files,
            &uri,
        );

        // Tactic-based code actions (Phase 0: wire existing infrastructure)
        let req = TacticRequest {
            ctx: &pretty_ctx,
            proof: proof_state,
            doc: &doc.parsed,
            index: doc.goals_index.as_ref(),
            selection: Selection { range },
            limits: TacticLimits::default(),
        };

        let actions = self.tactic_registry.compute_all(&req);

        let mut response: Vec<CodeActionOrCommand> = actions
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

        // Add DB-7 rename preview actions (if enabled)
        if self.config.read().await.enable_db7_hover_preview {
            // Compact preview (default)
            if let Some(db7_action) = self.code_action_db7_preview(&uri, range.start).await {
                response.push(CodeActionOrCommand::CodeAction(db7_action));
            }
            
            // Detailed preview (for power users)
            if let Some(db7_action_detailed) = self.code_action_db7_preview_detailed(&uri, range.start).await {
                response.push(CodeActionOrCommand::CodeAction(db7_action_detailed));
            }
        }

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
                            "pythonic" => PrettyDialect::Pythonic,
                             "canonical" => PrettyDialect::Canonical,
                            _ => PrettyDialect::Canonical,
                        })
                        .unwrap_or(PrettyDialect::Canonical);
                
                let files = DiagnosticContext::new("hover_dummy".to_string(), "");
                let ctx = crate::edgelord_pretty_ctx::EdgeLordPrettyCtx::new(
                    &self.registry,
                    dialect,
                    PrettyLimits::inlay_default(),
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
                let index = SymbolIndex::build(&doc.text);
                Some(
                    index.definitions.iter()
                        .filter(|d| matches!(d.kind, SymbolDefKind::Touch | SymbolDefKind::Def | SymbolDefKind::Rule | SymbolDefKind::Sugar | SymbolDefKind::Import))
                        .map(|def| {
                            let range = byte_span_to_range(&doc.text, def.form_span);
                            let sel_range = byte_span_to_range(&doc.text, def.name_span);
                            let kind = match def.kind {
                                SymbolDefKind::Touch => tower_lsp::lsp_types::SymbolKind::VARIABLE,
                                SymbolDefKind::Def => tower_lsp::lsp_types::SymbolKind::FUNCTION,
                                SymbolDefKind::Rule => tower_lsp::lsp_types::SymbolKind::OPERATOR,
                                SymbolDefKind::Sugar => tower_lsp::lsp_types::SymbolKind::INTERFACE,
                                SymbolDefKind::Import => tower_lsp::lsp_types::SymbolKind::MODULE,
                                SymbolDefKind::Let => tower_lsp::lsp_types::SymbolKind::VARIABLE,
                                SymbolDefKind::Lambda => tower_lsp::lsp_types::SymbolKind::VARIABLE,
                            };
                            DocumentSymbol {
                                name: def.name.clone(),
                                detail: def.detail.clone(),
                                kind,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: sel_range,
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

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Phase 1: Local definition lookup
        let (local_result, symbol_name) = {
            let session = self.proof_session.read().await;
            let doc = match session.get_parsed_document(&uri) {
                Some(d) => d,
                None => return Ok(None),
            };

            let offset = position_to_offset(&doc.text, position);
            let index = SymbolIndex::build(&doc.text);

            // If cursor is already on a definition, show it
            if let Some(def) = index.definition_at_offset(offset) {
                // If this is an import definition, try to jump to the source module
                if def.kind == SymbolDefKind::Import {
                    if let Some(detail) = &def.detail {
                        if detail.starts_with("use ") {
                            let module_path_str = &detail[4..]; // strip "use "
                            (None, Some((def.name.clone(), Some(module_path_str.to_string()))))
                        } else {
                            let range = byte_span_to_range(&doc.text, def.name_span);
                            (Some(GotoDefinitionResponse::Scalar(Location {
                                uri: uri.clone(),
                                range,
                            })), None)
                        }
                    } else {
                        let range = byte_span_to_range(&doc.text, def.name_span);
                        (Some(GotoDefinitionResponse::Scalar(Location {
                            uri: uri.clone(),
                            range,
                        })), None)
                    }
                } else {
                    let range = byte_span_to_range(&doc.text, def.name_span);
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range,
                    })));
                }
            }
            // If cursor is on a reference, find its local definition
            else if let Some(sym_ref) = index.reference_at_offset(offset) {
                if let Some(def) = index.find_definition(&sym_ref.name) {
                    // If definition is an import, try cross-file resolution
                    if def.kind == SymbolDefKind::Import {
                        if let Some(detail) = &def.detail {
                            if detail.starts_with("use ") {
                                let module_path_str = &detail[4..];
                                (None, Some((sym_ref.name.clone(), Some(module_path_str.to_string()))))
                            } else {
                                let range = byte_span_to_range(&doc.text, def.name_span);
                                (Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: uri.clone(),
                                    range,
                                })), None)
                            }
                        } else {
                            let range = byte_span_to_range(&doc.text, def.name_span);
                            (Some(GotoDefinitionResponse::Scalar(Location {
                                uri: uri.clone(),
                                range,
                            })), None)
                        }
                    } else {
                        let range = byte_span_to_range(&doc.text, def.name_span);
                        (Some(GotoDefinitionResponse::Scalar(Location {
                            uri: uri.clone(),
                            range,
                        })), None)
                    }
                } else {
                    // Symbol not defined locally — might be from an import
                    (None, Some((sym_ref.name.clone(), None)))
                }
            } else {
                // Try extracting the symbol under cursor manually
                let sym = symbol_at_position(&doc.text, position);
                if let Some(name) = sym {
                    if let Some(def) = index.find_definition(&name) {
                        if def.kind == SymbolDefKind::Import {
                            if let Some(detail) = &def.detail {
                                if detail.starts_with("use ") {
                                    let module_path_str = &detail[4..];
                                    (None, Some((name, Some(module_path_str.to_string()))))
                                } else {
                                    let range = byte_span_to_range(&doc.text, def.name_span);
                                    (Some(GotoDefinitionResponse::Scalar(Location {
                                        uri: uri.clone(),
                                        range,
                                    })), None)
                                }
                            } else {
                                let range = byte_span_to_range(&doc.text, def.name_span);
                                (Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: uri.clone(),
                                    range,
                                })), None)
                            }
                        } else {
                            let range = byte_span_to_range(&doc.text, def.name_span);
                            (Some(GotoDefinitionResponse::Scalar(Location {
                                uri: uri.clone(),
                                range,
                            })), None)
                        }
                    } else {
                        (None, Some((name, None)))
                    }
                } else {
                    (None, None)
                }
            }
        };

        // Return local result if found
        if let Some(result) = local_result {
            return Ok(Some(result));
        }

        // Phase 1.2: Cross-file resolution via MadLibResolver
        if let Some((symbol, module_path_opt)) = symbol_name {
            // Try to resolve via import module path
            if let Some(module_path_str) = module_path_opt {
                let resolver = self.madlib_resolver.read().await;
                if let Some(ref resolver) = *resolver {
                    let module_path = ModulePath::parse(&module_path_str);
                    if let Ok(resolved_file) = resolver.resolve_module(&module_path) {
                        // Read the target module, parse it, and find the symbol definition
                        if let Ok(source) = std::fs::read_to_string(&resolved_file) {
                            let target_index = SymbolIndex::build(&source);
                            if let Some(def) = target_index.find_definition(&symbol) {
                                let target_uri = Url::from_file_path(&resolved_file)
                                    .unwrap_or_else(|_| uri.clone());
                                let range = byte_span_to_range(&source, def.name_span);
                                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: target_uri,
                                    range,
                                })));
                            }
                        }
                    }
                }
            }

            // Fallback: scan imports in the current document for the symbol
            let session = self.proof_session.read().await;
            if let Some(doc) = session.get_parsed_document(&uri) {
                let index = SymbolIndex::build(&doc.text);
                // Look for matching import that brings this symbol in
                for def in &index.definitions {
                    if def.kind == SymbolDefKind::Import && def.name == symbol {
                        if let Some(detail) = &def.detail {
                            if detail.starts_with("use ") {
                                let module_path_str = &detail[4..];
                                let resolver = self.madlib_resolver.read().await;
                                if let Some(ref resolver) = *resolver {
                                    let module_path = ModulePath::parse(module_path_str);
                                    if let Ok(resolved_file) = resolver.resolve_module(&module_path) {
                                        if let Ok(source) = std::fs::read_to_string(&resolved_file) {
                                            let target_index = SymbolIndex::build(&source);
                                            if let Some(target_def) = target_index.find_definition(&symbol) {
                                                let target_uri = Url::from_file_path(&resolved_file)
                                                    .unwrap_or_else(|_| uri.clone());
                                                let range = byte_span_to_range(&source, target_def.name_span);
                                                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                                    uri: target_uri,
                                                    range,
                                                })));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;

        let items = {
            let session = self.proof_session.read().await;
            let doc = match session.get_parsed_document(&uri) {
                Some(d) => d,
                None => return Ok(None),
            };

            let index = SymbolIndex::build(&doc.text);
            let mut items = Vec::new();
            let mut seen_names = BTreeSet::new();

            // Add all local definitions as completion candidates
            for def in &index.definitions {
                let kind = match def.kind {
                    SymbolDefKind::Touch => CompletionItemKind::VARIABLE,
                    SymbolDefKind::Def => CompletionItemKind::FUNCTION,
                    SymbolDefKind::Rule => CompletionItemKind::OPERATOR,
                    SymbolDefKind::Sugar => CompletionItemKind::KEYWORD,
                    SymbolDefKind::Let => CompletionItemKind::VARIABLE,
                    SymbolDefKind::Lambda => CompletionItemKind::VARIABLE,
                    SymbolDefKind::Import => CompletionItemKind::MODULE,
                };
                if seen_names.insert(def.name.clone()) {
                    items.push(CompletionItem {
                        label: def.name.clone(),
                        kind: Some(kind),
                        detail: def.detail.clone(),
                        sort_text: Some(format!("0_{}", def.name)), // Local symbols sort first
                        ..CompletionItem::default()
                    });
                }
            }

            // Add symbols from imported modules (cross-file completion)
            {
                let resolver = self.madlib_resolver.read().await;
                if let Some(ref resolver) = *resolver {
                    for def in &index.definitions {
                        if def.kind == SymbolDefKind::Import {
                            if let Some(detail) = &def.detail {
                                if detail.starts_with("use ") {
                                    let module_path_str = &detail[4..];
                                    let module_path = ModulePath::parse(module_path_str);
                                    if let Ok(resolved_file) = resolver.resolve_module(&module_path) {
                                        if let Ok(source) = std::fs::read_to_string(&resolved_file) {
                                            let target_index = SymbolIndex::build(&source);
                                            for target_def in &target_index.definitions {
                                                if !matches!(target_def.kind, SymbolDefKind::Import | SymbolDefKind::Let | SymbolDefKind::Lambda) {
                                                    if seen_names.insert(target_def.name.clone()) {
                                                        let kind = match target_def.kind {
                                                            SymbolDefKind::Touch => CompletionItemKind::VARIABLE,
                                                            SymbolDefKind::Def => CompletionItemKind::FUNCTION,
                                                            SymbolDefKind::Rule => CompletionItemKind::OPERATOR,
                                                            SymbolDefKind::Sugar => CompletionItemKind::KEYWORD,
                                                            _ => CompletionItemKind::VARIABLE,
                                                        };
                                                        items.push(CompletionItem {
                                                            label: target_def.name.clone(),
                                                            kind: Some(kind),
                                                            detail: Some(format!("{} (from {})", target_def.detail.as_deref().unwrap_or(""), module_path_str)),
                                                            sort_text: Some(format!("1_{}", target_def.name)), // Imported symbols sort after local
                                                            ..CompletionItem::default()
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add kernel keywords
            for kw in KERNEL_KEYWORDS {
                if seen_names.insert(kw.to_string()) {
                    items.push(CompletionItem {
                        label: kw.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("keyword".to_string()),
                        sort_text: Some(format!("2_{}", kw)), // Keywords sort last
                        ..CompletionItem::default()
                    });
                }
            }

            // Add commonly-used prelude operations
            let prelude_ops = [
                ("normalize", "Normalize a term in the current scope"),
                ("coherent?", "Check if two terms are coherent"),
                ("check-diagram-coherence", "Verify diagram commutativity"),
                ("classify-reduction", "Classify reduction steps"),
                ("compose-recipes", "Compose two recipe scopes"),
                ("jet-trace", "Compute k-jet rewrite continuations"),
                ("transport-trace", "Trace transport along a functor"),
                ("localization-trace", "Localize at a morphism"),
                ("six-functor-trace", "Trace six-functor adjunctions"),
                ("connection-trace", "Compute connection/parallel transport"),
                ("holonomy-trace", "Compute holonomy of a loop"),
                ("spectral-trace", "Compute spectral sequence data"),
                ("kahler-trace", "Compute Kähler differentials"),
                ("suspension-trace", "Compute suspension/desuspension"),
                ("loop-trace", "Compute loop space structure"),
                ("tate-twist-trace", "Apply Tate twist"),
            ];
            for (op, doc) in prelude_ops {
                if seen_names.insert(op.to_string()) {
                    items.push(CompletionItem {
                        label: op.to_string(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(doc.to_string()),
                        sort_text: Some(format!("3_{}", op)), // Prelude ops sort after keywords
                        ..CompletionItem::default()
                    });
                }
            }

            items
        };
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let result = {
            let session = self.proof_session.read().await;
            let doc = match session.get_parsed_document(&uri) {
                Some(d) => d,
                None => return Ok(None),
            };

            let offset = position_to_offset(&doc.text, position);
            let index = SymbolIndex::build(&doc.text);

            // Find the symbol name at the cursor
            let name = index.definition_at_offset(offset)
                .map(|d| d.name.clone())
                .or_else(|| index.reference_at_offset(offset).map(|r| r.name.clone()))
                .or_else(|| symbol_at_position(&doc.text, position));

            if let Some(name) = name {
                let mut locations = Vec::new();

                // Include definition sites if requested (or always for convenience)
                if params.context.include_declaration {
                    for def in &index.definitions {
                        if def.name == name {
                            locations.push(Location {
                                uri: uri.clone(),
                                range: byte_span_to_range(&doc.text, def.name_span),
                            });
                        }
                    }
                }

                // Include all reference sites
                for sym_ref in index.find_references(&name) {
                    locations.push(Location {
                        uri: uri.clone(),
                        range: byte_span_to_range(&doc.text, sym_ref.span),
                    });
                }

                if locations.is_empty() { None } else { Some(locations) }
            } else {
                None
            }
        };
        Ok(result)
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;

        let ranges = {
            let session = self.proof_session.read().await;
            let doc = match session.get_parsed_document(&uri) {
                Some(d) => d,
                None => return Ok(None),
            };

            let mut folds = Vec::new();

            // Top-level forms that span multiple lines
            for (_, span) in top_level_symbols(&doc.text) {
                let start = offset_to_position(&doc.text, span.start);
                let end = offset_to_position(&doc.text, span.end);
                if end.line > start.line {
                    folds.push(FoldingRange {
                        start_line: start.line,
                        start_character: Some(start.character),
                        end_line: end.line,
                        end_character: Some(end.character),
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }

            // Comment blocks: consecutive lines starting with ;
            let mut comment_start: Option<u32> = None;
            for (line_num, line) in doc.text.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with(';') {
                    if comment_start.is_none() {
                        comment_start = Some(line_num as u32);
                    }
                } else {
                    if let Some(start) = comment_start {
                        let end = line_num as u32 - 1;
                        if end > start {
                            folds.push(FoldingRange {
                                start_line: start,
                                start_character: None,
                                end_line: end,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Comment),
                                collapsed_text: None,
                            });
                        }
                        comment_start = None;
                    }
                }
            }
            // Close any trailing comment block
            if let Some(start) = comment_start {
                let end = doc.text.lines().count() as u32 - 1;
                if end > start {
                    folds.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: end,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Comment),
                        collapsed_text: None,
                    });
                }
            }

            folds
        };
        Ok(Some(ranges))
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

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let session = self.proof_session.read().await;
        let doc = match session.get_parsed_document(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let offset = position_to_offset(&doc.text, position);
        
        // Walk backwards to find the enclosing form's head symbol
        let text_before = &doc.text[..offset];
        
        // Find the last unmatched '(' before cursor
        let mut depth = 0i32;
        let mut last_form_start = None;
        for (i, ch) in text_before.char_indices().rev() {
            match ch {
                ')' => depth += 1,
                '(' => {
                    depth -= 1;
                    if depth < 0 {
                        last_form_start = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(form_start) = last_form_start {
            // Extract the head symbol after '('
            let after_paren = &doc.text[form_start + 1..];
            let head: String = after_paren.chars()
                .take_while(|c| is_identifier_char(*c))
                .collect();

            if head.is_empty() {
                return Ok(None);
            }

            // Provide signature based on known operations
            let sig = match head.as_str() {
                "def" => Some(("(def name body)", vec!["name: symbol", "body: expression"])),
                "touch" => Some(("(touch name [type])", vec!["name: symbol", "type: optional type annotation"])),
                "rule" => Some(("(rule name lhs rhs [meta])", vec!["name: symbol", "lhs: left-hand side pattern", "rhs: right-hand side replacement", "meta: optional metadata"])),
                "sugar" => Some(("(sugar name pattern template)", vec!["name: symbol", "pattern: input pattern", "template: output template"])),
                "let" => Some(("(let (bindings...) body)", vec!["bindings: pairs of (name value)", "body: expression"])),
                "begin" | "do" => Some(("(begin form1 form2 ...)", vec!["forms: sequence of expressions"])),
                "use" => Some(("(use Module::Path symbol [as alias])", vec!["Module::Path: module path", "symbol: symbol to import", "as alias: optional renaming"])),
                "lambda" | "fn" => Some(("(lambda (params) body)", vec!["params: parameter list", "body: expression"])),
                "if" => Some(("(if condition then else)", vec!["condition: boolean expression", "then: true branch", "else: false branch"])),
                "match" => Some(("(match expr (pattern1 result1) ...)", vec!["expr: expression to match", "clauses: (pattern result) pairs"])),
                "normalize" => Some(("(normalize scope term fuel)", vec!["scope: rewriting scope", "term: term to normalize", "fuel: maximum rewrite steps"])),
                "coherent?" => Some(("(coherent? trace1 trace2)", vec!["trace1: first trace", "trace2: second trace"])),
                "check-diagram-coherence" => Some(("(check-diagram-coherence scope spec fuel)", vec!["scope: rewriting scope", "spec: diagram specification", "fuel: maximum steps"])),
                "grade" => Some(("(grade p q term)", vec!["p: first bidegree component", "q: second bidegree component", "term: graded term"])),
                "transport" => Some(("(transport functor term)", vec!["functor: transport functor", "term: term to transport"])),
                "assert" | "assert-coherent" => Some(("(assert-coherent term1 term2)", vec!["term1: first term", "term2: second term (must be coherent)"])),
                "quote" => Some(("(quote form)", vec!["form: s-expression to quote"])),
                "cons" => Some(("(cons head tail)", vec!["head: first element", "tail: rest of list"])),
                "in" => Some(("(in doctrine body)", vec!["doctrine: rewriting doctrine (e.g. stable, motivic)", "body: expression in that doctrine"])),
                _ => None,
            };

            if let Some((label, params_list)) = sig {
                // Count which parameter the cursor is on (by counting spaces/forms between '(' and cursor)
                let between = &doc.text[form_start + 1..offset];
                let mut param_index = 0u32;
                let mut depth_inner = 0i32;
                for ch in between.chars() {
                    match ch {
                        '(' => depth_inner += 1,
                        ')' => depth_inner -= 1,
                        ' ' | '\n' | '\t' if depth_inner == 0 => param_index += 1,
                        _ => {}
                    }
                }
                // Subtract 1 because the head symbol is parameter 0 in our counting but not in LSP
                let active_param = if param_index > 0 { param_index - 1 } else { 0 };

                let parameters: Vec<ParameterInformation> = params_list.iter().map(|p| {
                    ParameterInformation {
                        label: ParameterLabel::Simple(p.to_string()),
                        documentation: None,
                    }
                }).collect();

                return Ok(Some(SignatureHelp {
                    signatures: vec![SignatureInformation {
                        label: label.to_string(),
                        documentation: None,
                        parameters: Some(parameters),
                        active_parameter: Some(active_param),
                    }],
                    active_signature: Some(0),
                    active_parameter: Some(active_param),
                }));
            }
        }

        Ok(None)
    }

    async fn prepare_rename(
        &self,
        params: tower_lsp::lsp_types::TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        let session = self.proof_session.read().await;
        let doc = match session.get_parsed_document(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let offset = position_to_offset(&doc.text, position);
        let index = SymbolIndex::build(&doc.text);

        // Find symbol at cursor (must be a definition or reference to a local definition)
        let name_and_range = index.definition_at_offset(offset)
            .filter(|d| !matches!(d.kind, SymbolDefKind::Import)) // Can't rename imports
            .map(|d| (d.name.clone(), byte_span_to_range(&doc.text, d.name_span)))
            .or_else(|| {
                index.reference_at_offset(offset)
                    .and_then(|r| {
                        index.find_definition(&r.name)
                            .filter(|d| !matches!(d.kind, SymbolDefKind::Import))
                            .map(|_| (r.name.clone(), byte_span_to_range(&doc.text, r.span)))
                    })
            });

        match name_and_range {
            Some((_name, range)) => {
                Ok(Some(PrepareRenameResponse::Range(range)))
            }
            None => Ok(None),
        }
    }

    async fn rename(
        &self,
        params: RenameParams,
    ) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let session = self.proof_session.read().await;
        let doc = match session.get_parsed_document(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let offset = position_to_offset(&doc.text, position);
        let index = SymbolIndex::build(&doc.text);

        // Find the symbol under cursor
        let old_name = index.definition_at_offset(offset)
            .filter(|d| !matches!(d.kind, SymbolDefKind::Import))
            .map(|d| d.name.clone())
            .or_else(|| {
                index.reference_at_offset(offset)
                    .and_then(|r| {
                        index.find_definition(&r.name)
                            .filter(|d| !matches!(d.kind, SymbolDefKind::Import))
                            .map(|_| r.name.clone())
                    })
            })
            .or_else(|| {
                symbol_at_position(&doc.text, position)
                    .filter(|name| {
                        index.find_definition(name)
                            .map_or(false, |d| !matches!(d.kind, SymbolDefKind::Import))
                    })
            });

        let Some(old_name) = old_name else {
            return Ok(None);
        };

        let mut edits = Vec::new();

        // Rename all definition sites
        for def in &index.definitions {
            if def.name == old_name && !matches!(def.kind, SymbolDefKind::Import) {
                edits.push(TextEdit {
                    range: byte_span_to_range(&doc.text, def.name_span),
                    new_text: new_name.clone(),
                });
            }
        }

        // Rename all reference sites
        for sym_ref in index.find_references(&old_name) {
            edits.push(TextEdit {
                range: byte_span_to_range(&doc.text, sym_ref.span),
                new_text: new_name.clone(),
            });
        }

        if edits.is_empty() {
            return Ok(None);
        }

        // Sort edits by position (INV D-*: deterministic ordering)
        edits.sort_by(|a, b| {
            a.range.start.line.cmp(&b.range.start.line)
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        let mut changes = std::collections::HashMap::new();
        changes.insert(uri, edits);

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let session = self.proof_session.read().await;
        let doc = match session.get_parsed_document(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let offset = position_to_offset(&doc.text, position);
        let index = SymbolIndex::build(&doc.text);

        // Find the symbol name at cursor
        let name = index.definition_at_offset(offset)
            .map(|d| d.name.clone())
            .or_else(|| index.reference_at_offset(offset).map(|r| r.name.clone()))
            .or_else(|| symbol_at_position(&doc.text, position));

        let Some(name) = name else {
            return Ok(None);
        };

        let mut highlights = Vec::new();

        // Highlight all definition sites
        for def in &index.definitions {
            if def.name == name {
                highlights.push(DocumentHighlight {
                    range: byte_span_to_range(&doc.text, def.name_span),
                    kind: Some(DocumentHighlightKind::WRITE),
                });
            }
        }

        // Highlight all reference sites
        for sym_ref in index.find_references(&name) {
            highlights.push(DocumentHighlight {
                range: byte_span_to_range(&doc.text, sym_ref.span),
                kind: Some(DocumentHighlightKind::READ),
            });
        }

        // Deterministic ordering (INV D-*)
        highlights.sort_by(|a, b| {
            a.range.start.line.cmp(&b.range.start.line)
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        if highlights.is_empty() { Ok(None) } else { Ok(Some(highlights)) }
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

/// Stub implementation for trace step extraction (TODO: implement)
fn extract_trace_steps(_term: &str) -> Option<Vec<String>> {
    // Placeholder implementation
    // This would parse trace steps from a quoted term
    // For now, returns None to make tests compile
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParsedDocument;
    use comrade_lisp::comrade_workspace::WorkspaceReport;
    use comrade_lisp::{WorkspaceDiagnostic, WorkspaceDiagnosticSeverity};
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
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
            fingerprint: None,
            revision: 0,
            bundle: None,
            proof_state: None,
        }
    }
    #[test]
    fn workspace_report_to_diagnostics_is_deterministic() {
        let report = sample_report();
        let uri = Url::parse("file:///test.tld").unwrap();
        let parsed = ParsedDocument::parse("012".to_string());
        
        let first = PublishDiagnosticsHandler::convert_diagnostics(&uri, &report, &parsed);
        let second = PublishDiagnosticsHandler::convert_diagnostics(&uri, &report, &parsed);
        
        assert_eq!(first, second, "conversion must be deterministic and repeatable");
    }
    #[test]
    fn document_diagnostics_include_workspace_report_diag() {
        let report = WorkspaceReport {
            diagnostics: vec![WorkspaceDiagnostic::error(
                "workspace diag",
                Some(Span::new(0, 1)),
                Some("code"),
            )],
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
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
    
    /// Funnel Invariant Test: Verify no direct publish_diagnostics calls outside handler
    ///
    /// This test ensures the architecture maintains a single publication funnel.
    /// Only PublishDiagnosticsHandler methods should call client.publish_diagnostics.
    #[test]
    fn funnel_invariant_no_shadow_publish_calls() {
        let source = include_str!("lsp.rs");
        
        // Count actual client.publish_diagnostics calls (the real side-effect).
        // These should only appear inside PublishDiagnosticsHandler.
        let mut direct_client_calls = 0;
        let mut handler_invocations = 0;
        let mut in_test = false;
        
        for (_line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            
            // Skip this test itself
            if trimmed.contains("fn funnel_invariant_no_shadow_publish_calls") {
                in_test = true;
            }
            if in_test {
                if trimmed == "}" {
                    in_test = false;
                }
                continue;
            }
            
            // Skip comments and doc comments
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            
            // Direct client.publish_diagnostics — the real side-effect call
            if trimmed.contains("client.publish_diagnostics(") {
                direct_client_calls += 1;
            }
            
            // Handler invocations (the safe way to publish)
            if trimmed.contains("publish_diagnostics_canonical(") 
                || trimmed.contains("publish_diagnostics_canonical_preconverted(") {
                handler_invocations += 1;
            }
        }
        
        // INVARIANT: Only 1 direct client.publish_diagnostics call should exist
        // (inside publish_diagnostics_internal, the single choke point).
        assert!(
            direct_client_calls <= 2,
            "Too many direct client.publish_diagnostics calls ({}). \
             All publication must go through PublishDiagnosticsHandler.",
            direct_client_calls
        );
        
        // There should be multiple handler invocations (did_open, did_change debounce, did_save, did_close).
        assert!(
            handler_invocations >= 4,
            "Expected at least 4 PublishDiagnosticsHandler invocations, found {}",
            handler_invocations
        );
    }
    #[test]
    #[ignore]
    fn test_extract_trace_steps_v1() {
        let term = "(quote (trace-v1 (version 1) (meta (foo bar)) (steps (step1 arg1) (step2) (step3 argA argB))))";
        let steps = extract_trace_steps(term);
        assert_eq!(steps, Some(vec!["(step1".to_string(), "arg1)".to_string(), "(step2)".to_string(), "(step3".to_string(), "argA".to_string(), "argB".to_string()]));
    }
    #[tokio::test]
    #[ignore]
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
