use edgelord_lsp::{Backend, lsp::Config}; // Adjusted imports
use tower_lsp::{LspService, Server};
use tokio::sync::RwLock;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let config = Arc::new(RwLock::new(Config::default())); // Define config outside closure

    let (service, socket) = LspService::new(|client| {
        // Pass the direct tower_lsp::Client and config to Backend::new
        Backend::new(client, config.clone()) // Use config.clone() here
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}