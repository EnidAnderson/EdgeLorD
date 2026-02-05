use std::{collections::BTreeMap, sync::Arc};
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

pub struct ProofDocument {
    pub version: i32,
    pub parsed: ParsedDocument,
    pub last_analyzed: Instant,
    pub workspace_report: WorkspaceReport,
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
}

impl ProofSession {
    pub fn new(client: Client, config: Arc<RwLock<Config>>) -> Self { // Changed client type
        Self {
            client,
            config,
            workspace: ComradeWorkspace::new(),
            documents: BTreeMap::new(),
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
        let goals = parsed.goals.clone();

        self.documents.insert(uri, ProofDocument {
            version,
            parsed,
            last_analyzed: Instant::now(),
            workspace_report: report.clone(),
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
                report: WorkspaceReport { diagnostics: Vec::new(), fingerprint: None, revision: 0, bundle: None },
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
        let goals = parsed.goals.clone();

        self.documents.insert(uri, ProofDocument {
            version,
            parsed,
            last_analyzed: Instant::now(),
            workspace_report: report.clone(),
        });

        ProofSessionUpdateResult {
            report,
            diagnostics,
            goals,
        }
    }

    pub fn get_goals(&self, uri: &Url) -> Vec<Goal> {
        self.documents
            .get(uri)
            .map(|doc| doc.parsed.goals.clone())
            .unwrap_or_default()
    }

    pub async fn apply_command(&mut self, uri: Url, _command: String) -> ProofSessionUpdateResult {
        let Some(current_proof_doc) = self.documents.get(&uri) else {
            // Updated call to log_message to use direct Client method
            self.client.log_message(MessageType::ERROR, format!("ProofSession: Attempted to apply command to non-existent document: {}", uri).to_string()).await;
            return ProofSessionUpdateResult {
                report: WorkspaceReport { diagnostics: Vec::new(), fingerprint: None, revision: 0, bundle: None },
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

    pub fn close(&mut self, uri: &Url) {
        if let Some(_) = self.documents.remove(uri) {
            self.workspace.did_close(&uri.to_string());
        }
    }
}
