use std::{collections::BTreeMap, sync::Arc};

use tokio::sync::RwLock;
use tower_lsp::{
    Client, LanguageServer,
    jsonrpc::Result,
    lsp_types::{
        CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic, DiagnosticSeverity,
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        DidSaveTextDocumentParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
        Hover, HoverContents, HoverParams, InlayHint, InlayHintKind, InlayHintParams,
        InitializeParams, InitializeResult, InitializedParams, MessageType, OneOf, Position,
        PublishDiagnosticsParams, Range, SelectionRange, SelectionRangeParams, ServerCapabilities,
        ServerInfo, TextDocumentContentChangeEvent, TextDocumentSyncCapability,
        TextDocumentSyncKind, Url, WorkDoneProgressOptions,
    },
};

use crate::document::{
    Binding, BindingKind, ByteSpan, ParsedDocument, apply_content_changes, offset_to_position,
    position_to_offset, top_level_symbols,
};

#[derive(Debug, Clone)]
struct DocumentState {
    version: i32,
    parsed: ParsedDocument,
}

#[derive(Default)]
struct ServerState {
    documents: BTreeMap<Url, DocumentState>,
}

pub struct Backend {
    client: Client,
    state: Arc<RwLock<ServerState>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(ServerState::default())),
        }
    }

    async fn upsert_document(&self, uri: Url, version: i32, text: String) {
        let parsed = ParsedDocument::parse(text);

        let diagnostics = to_lsp_diagnostics(&parsed);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, Some(version))
            .await;

        let mut state = self.state.write().await;
        state.documents.insert(uri, DocumentState { version, parsed });
    }

    async fn with_document<T>(&self, uri: &Url, f: impl FnOnce(&DocumentState) -> T) -> Option<T> {
        let state = self.state.read().await;
        state.documents.get(uri).map(f)
    }

    async fn replace_with_changes(
        &self,
        uri: Url,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        let base_text = self
            .with_document(&uri, |doc| doc.parsed.text.clone())
            .await
            .unwrap_or_default();

        let text = apply_content_changes(&base_text, &changes);

        self.upsert_document(uri, version, text).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
                code_action_provider: Some(tower_lsp::lsp_types::CodeActionProviderCapability::Simple(
                    true,
                )),
                inlay_hint_provider: Some(OneOf::Left(true)),
                diagnostic_provider: Some(tower_lsp::lsp_types::DiagnosticServerCapabilities::Options(
                    tower_lsp::lsp_types::DiagnosticOptions {
                        identifier: Some("edgelord-lsp".into()),
                        inter_file_dependencies: false,
                        workspace_diagnostics: false,
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                    },
                )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "edgelord-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.upsert_document(
            params.text_document.uri,
            params.text_document.version,
            params.text_document.text,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.replace_with_changes(
            params.text_document.uri,
            params.text_document.version,
            params.content_changes,
        )
        .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let version = self
            .with_document(&params.text_document.uri, |doc| doc.version)
            .await
            .unwrap_or(0);

        if let Some(text) = params.text {
            self.upsert_document(params.text_document.uri, version, text).await;
        } else if let Some(diags) = self
            .with_document(&params.text_document.uri, |doc| to_lsp_diagnostics(&doc.parsed))
            .await
        {
            self.client
                .publish_diagnostics(params.text_document.uri, diags, Some(version))
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut state = self.state.write().await;
        state.documents.remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let Some(markdown) = self
            .with_document(&uri, |doc| {
                let offset = position_to_offset(&doc.parsed.text, position);
                if let Some(goal) = doc.parsed.goal_at_offset(offset) {
                    let goal_name = goal.name.as_deref().unwrap_or("?");
                    let ctx = format_context(&goal.context, 8);
                    return Some(format!(
                        "**Goal** `{}`\n\n- id: `{}`\n- target: `{}`\n- context: {}",
                        goal_name, goal.goal_id, goal.target, ctx
                    ));
                }
                let chain = doc.parsed.selection_chain_for_offset(offset);
                chain.first().map(|span| {
                    format!(
                        "Focused span: [{}..{}]",
                        span.start,
                        span.end
                    )
                })
            })
            .await
            .flatten()
        else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(tower_lsp::lsp_types::MarkupContent {
                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn code_action(&self, _: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        // MVP0: keep API surface wired; candidate generation lands in MVP2.
        Ok(Some(Vec::<CodeActionOrCommand>::new()))
    }

    async fn selection_range(&self, params: SelectionRangeParams) -> Result<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        let Some(ranges) = self
            .with_document(&uri, |doc| {
                params
                    .positions
                    .iter()
                    .map(|position| {
                        let offset = position_to_offset(&doc.parsed.text, *position);
                        let chain = doc.parsed.selection_chain_for_offset(offset);
                        chain_to_selection_range(&doc.parsed.text, &chain)
                    })
                    .collect::<Vec<_>>()
            })
            .await
        else {
            return Ok(None);
        };

        Ok(Some(ranges))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let Some(hints) = self
            .with_document(&uri, |doc| {
                let start = position_to_offset(&doc.parsed.text, params.range.start);
                let end = position_to_offset(&doc.parsed.text, params.range.end);
                let query = ByteSpan::new(start.min(end), start.max(end));
                doc.parsed
                    .goal_inlay_hints_in_range(query)
                    .into_iter()
                    .map(|hint| InlayHint {
                        position: offset_to_position(&doc.parsed.text, hint.offset),
                        label: hint.label.into(),
                        kind: Some(InlayHintKind::TYPE),
                        text_edits: None,
                        tooltip: None,
                        padding_left: Some(true),
                        padding_right: Some(false),
                        data: None,
                    })
                    .collect::<Vec<_>>()
            })
            .await
        else {
            return Ok(None);
        };

        Ok(Some(hints))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(symbols) = self
            .with_document(&uri, |doc| {
                top_level_symbols(&doc.parsed.text)
                    .into_iter()
                    .map(|(name, span)| {
                        let range = byte_span_to_range(&doc.parsed.text, span);
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
                    .collect::<Vec<_>>()
            })
            .await
        else {
            return Ok(None);
        };

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn diagnostic(
        &self,
        params: tower_lsp::lsp_types::DocumentDiagnosticParams,
    ) -> Result<tower_lsp::lsp_types::DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        let items = self
            .with_document(&uri, |doc| to_lsp_diagnostics(&doc.parsed))
            .await
            .unwrap_or_default();

        Ok(tower_lsp::lsp_types::DocumentDiagnosticReportResult::Report(
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
        ))
    }
}

fn to_lsp_diagnostics(parsed: &ParsedDocument) -> Vec<Diagnostic> {
    let mut diagnostics = parsed
        .diagnostics
        .iter()
        .map(|diag| Diagnostic {
            range: byte_span_to_range(&parsed.text, diag.span),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(tower_lsp::lsp_types::NumberOrString::String("parse".into())),
            code_description: None,
            source: Some("edgelord-lsp".into()),
            message: diag.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        })
        .collect::<Vec<_>>();

    diagnostics.extend(parsed.goals.iter().map(|goal| {
        let name = goal.name.as_deref().unwrap_or("?");
        Diagnostic {
            range: byte_span_to_range(&parsed.text, goal.span),
            severity: Some(DiagnosticSeverity::INFORMATION),
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                "goal.unsolved".into(),
            )),
            code_description: None,
            source: Some("edgelord-lsp".into()),
            message: format!("Unsolved goal `{name}`"),
            related_information: None,
            tags: None,
            data: None,
        }
    }));

    diagnostics.sort_by(|a, b| {
        let a_key = (
            a.range.start.line,
            a.range.start.character,
            a.range.end.line,
            a.range.end.character,
            a.message.as_str(),
        );
        let b_key = (
            b.range.start.line,
            b.range.start.character,
            b.range.end.line,
            b.range.end.character,
            b.message.as_str(),
        );
        a_key.cmp(&b_key)
    });
    diagnostics
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
        format!("{}, … +{} more", shown.join(", "), bindings.len() - max_items)
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

#[allow(dead_code)]
fn _publish_diagnostics_params(
    uri: Url,
    diagnostics: Vec<Diagnostic>,
    version: Option<i32>,
) -> PublishDiagnosticsParams {
    PublishDiagnosticsParams {
        uri,
        diagnostics,
        version,
    }
}
