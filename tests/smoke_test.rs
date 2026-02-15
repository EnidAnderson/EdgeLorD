// Minimal smoke test to verify LSP server starts and responds
// This test is intentionally simple to isolate LSP loop issues

use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufStream, duplex};
use tokio::time::{timeout, Duration};
use tower_lsp::{LspService, Server, lsp_types::InitializeParams};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use edgelord_lsp::lsp::{Backend, Config};

// Helper to send JSON-RPC message
async fn send_message(stream: &mut BufStream<tokio::io::DuplexStream>, message: Value) {
    let message_str = serde_json::to_string(&message).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", message_str.len());
    
    eprintln!("SEND: {}", header.trim());
    eprintln!("SEND: {}", message_str);
    
    stream.write_all(header.as_bytes()).await.unwrap();
    stream.write_all(message_str.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}

// Helper to read ONE JSON-RPC message
async fn read_one_message(stream: &mut BufStream<tokio::io::DuplexStream>) -> Option<Value> {
    use tokio::io::AsyncBufReadExt;
    
    let mut content_length: Option<usize> = None;
    let mut buffer = String::new();

    // Read headers
    loop {
        buffer.clear();
        match timeout(Duration::from_secs(5), stream.read_line(&mut buffer)).await {
            Ok(Ok(0)) => {
                eprintln!("RECV: EOF");
                return None;
            }
            Ok(Ok(_)) => {
                eprintln!("RECV HEADER: {}", buffer.trim());
                
                if buffer == "\r\n" {
                    // End of headers
                    break;
                } else if buffer.starts_with("Content-Length:") {
                    content_length = Some(
                        buffer["Content-Length:".len()..]
                            .trim()
                            .parse()
                            .unwrap(),
                    );
                }
            }
            Ok(Err(e)) => {
                eprintln!("RECV ERROR: {:?}", e);
                return None;
            }
            Err(_) => {
                eprintln!("RECV TIMEOUT");
                return None;
            }
        }
    }

    // Read content
    if let Some(len) = content_length {
        use tokio::io::AsyncReadExt;
        let mut content_buffer = vec![0; len];
        match timeout(Duration::from_secs(5), stream.read_exact(&mut content_buffer)).await {
            Ok(Ok(_)) => {
                let message: Value = serde_json::from_slice(&content_buffer).unwrap();
                eprintln!("RECV BODY: {}", serde_json::to_string(&message).unwrap());
                Some(message)
            }
            Ok(Err(e)) => {
                eprintln!("RECV CONTENT ERROR: {:?}", e);
                None
            }
            Err(_) => {
                eprintln!("RECV CONTENT TIMEOUT");
                None
            }
        }
    } else {
        eprintln!("RECV: No Content-Length header");
        None
    }
}

#[tokio::test]
async fn smoke_test_server_starts() {
    eprintln!("\n=== SMOKE TEST: Server Starts ===\n");
    
    // Create duplex streams
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config::default()));

    eprintln!("Creating LspService...");
    let (service, socket) = LspService::new(|client| {
        eprintln!("Backend::new called");
        Backend::new(client, config_arc.clone())
    });

    eprintln!("Starting server...");
    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    // Client streams
    let mut client_stream = BufStream::new(client_to_server_tx);
    let mut server_stream = BufStream::new(server_to_client_rx);

    eprintln!("\n--- Sending initialize ---");
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": InitializeParams {
            capabilities: Default::default(),
            process_id: None,
            root_uri: None,
            root_path: None,
            client_info: None,
            locale: None,
            initialization_options: Some(serde_json::to_value(&Config::default()).unwrap()),
            trace: None,
            workspace_folders: None,
        },
    });
    send_message(&mut client_stream, initialize_request).await;

    eprintln!("\n--- Waiting for initialize response ---");
    
    // Server may send notifications (like window/logMessage) before the response
    // Keep reading until we get a message with id=1
    let mut response = None;
    for _ in 0..10 {
        if let Some(msg) = read_one_message(&mut server_stream).await {
            if msg.get("id").and_then(|id| id.as_i64()) == Some(1) {
                response = Some(msg);
                break;
            } else {
                eprintln!("Skipping notification: {}", msg.get("method").and_then(|m| m.as_str()).unwrap_or("unknown"));
            }
        } else {
            break;
        }
    }
    
    assert!(response.is_some(), "Server should respond to initialize");
    let response = response.unwrap();
    
    eprintln!("\n--- Got response ---");
    assert_eq!(response["id"], 1, "Response ID should match request ID");
    assert!(response["result"]["capabilities"].is_object(), "Should have capabilities");
    
    eprintln!("\n=== SMOKE TEST PASSED ===\n");
}

#[tokio::test]
async fn smoke_test_initialize_and_shutdown() {
    eprintln!("\n=== SMOKE TEST: Initialize and Shutdown ===\n");
    
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config::default()));
    let (service, socket) = LspService::new(|client| Backend::new(client, config_arc.clone()));
    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    let mut client_stream = BufStream::new(client_to_server_tx);
    let mut server_stream = BufStream::new(server_to_client_rx);

    // Initialize
    eprintln!("Sending initialize...");
    send_message(&mut client_stream, json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": InitializeParams {
            capabilities: Default::default(),
            process_id: None,
            root_uri: None,
            root_path: None,
            client_info: None,
            locale: None,
            initialization_options: Some(serde_json::to_value(&Config::default()).unwrap()),
            trace: None,
            workspace_folders: None,
        },
    })).await;

    // Read until we get the initialize response (skip notifications)
    let mut response = None;
    for _ in 0..10 {
        if let Some(msg) = read_one_message(&mut server_stream).await {
            if msg.get("id").and_then(|id| id.as_i64()) == Some(1) {
                response = Some(msg);
                break;
            }
        }
    }
    let response = response.expect("Initialize response");
    assert_eq!(response["id"], 1);

    // Initialized notification
    eprintln!("Sending initialized...");
    send_message(&mut client_stream, json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {},
    })).await;

    // Shutdown
    eprintln!("Sending shutdown...");
    send_message(&mut client_stream, json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown",
        "params": null,
    })).await;

    let response = read_one_message(&mut server_stream).await.expect("Shutdown response");
    assert_eq!(response["id"], 2);
    assert_eq!(response["result"], json!(null));

    eprintln!("\n=== SMOKE TEST PASSED ===\n");
}
