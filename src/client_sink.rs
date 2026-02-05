/*
use async_trait::async_trait;
use tower_lsp::
    lsp_types::{Diagnostic, MessageType, PublishDiagnosticsParams, Url};
use tower_lsp::Client;

#[async_trait] // Removed ?Send
pub trait ClientSink: Send + Sync + 'static {
    async fn log_message(&self, message_type: MessageType, message: String);
    async fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>, version: Option<i32>);
    // Add other methods that Backend needs to call on Client
}

pub struct RealClientSink {
    inner: Client,
}

impl RealClientSink {
    pub fn new(client: Client) -> Self {
        Self { inner: client }
    }
}

#[async_trait] // Removed ?Send
impl ClientSink for RealClientSink {
    async fn log_message(&self, message_type: MessageType, message: String) {
        self.inner.log_message(message_type, message).await;
    }

    async fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>, version: Option<i32>) {
        self.inner.publish_diagnostics(uri, diagnostics, version).await;
    }
}
*/
