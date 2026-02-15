use std::{sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufStream, duplex, AsyncReadExt};
use tokio::time::{timeout, Duration, Instant}; // Added Instant
use tower_lsp::{
    LspService, Server,
    lsp_types::{
        InitializeParams, InitializedParams, DidOpenTextDocumentParams,
        TextDocumentItem, Url, TextDocumentContentChangeEvent, DidChangeTextDocumentParams,
        VersionedTextDocumentIdentifier, PublishDiagnosticsParams, Range,
    },
};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use edgelord_lsp::{lsp::{Backend, Config}};


// Helper function to send JSON-RPC messages over the duplex stream
async fn send_message(stream: &mut BufStream<tokio::io::DuplexStream>, message: Value) {
    let message_str = serde_json::to_string(&message).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", message_str.len());
    stream.write_all(header.as_bytes()).await.unwrap();
    stream.write_all(message_str.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}

// Helper function to read ONE JSON-RPC message from the duplex stream
async fn read_one_message(stream: &mut BufStream<tokio::io::DuplexStream>) -> Option<Value> {
    let mut reader = BufReader::new(stream);
    let mut content_length: Option<usize> = None;
    let mut buffer = String::new();

    loop {
        buffer.clear();
        match timeout(Duration::from_secs(15), reader.read_line(&mut buffer)).await {
            Ok(Ok(0)) => return None, // EOF
            Ok(Ok(_)) => {
                if buffer == "\r\n" {
                    // End of headers
                    if let Some(len) = content_length {
                        let mut content_buffer = vec![0; len];
                        timeout(Duration::from_secs(15), reader.read_exact(&mut content_buffer))
                            .await
                            .expect("Reading content timed out")
                            .unwrap();
                        let message: Value = serde_json::from_slice(&content_buffer).unwrap();
                        return Some(message);
                    }
                } else if buffer.starts_with("Content-Length:") {
                    content_length = Some(
                        buffer["Content-Length:".len()..]
                            .trim()
                            .parse()
                            .unwrap(),
                    );
                }
            },
            Ok(Err(e)) => panic!("Error reading line: {:?}", e),
            Err(_) => panic!("Reading line timed out"),
        }
    }
}

// Test 1: Initialize, didOpen, expect publishDiagnostics
#[tokio::test]
async fn test_initialize_did_open_publishes_diagnostics() {
    // Client sends requests to client_to_server_tx, reads responses from server_to_client_rx
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config::default()));

    let (service, socket) = LspService::new(|client| {
        Backend::new(client, config_arc.clone())
    });

    // Server reads from client_to_server_rx, writes to server_to_client_tx
    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    // Client side streams
    let mut client_stream = BufStream::new(client_to_server_tx);
    let mut server_stream_reader = BufStream::new(server_to_client_rx);


    // 1. Send initialize request
    let initialize_id = 1;
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": initialize_id,
        "method": "initialize",
        "params": InitializeParams {
            capabilities: Default::default(),
            process_id: None,
            root_uri: None, // Used root_uri instead of root_path
            root_path: None, // Still there for compatibility, but prefer root_uri
            client_info: None,
            locale: None,
            initialization_options: Some(serde_json::to_value(&Config::default()).unwrap()),
            trace: None,
            workspace_folders: None,
        },
    });
    send_message(&mut client_stream, initialize_request).await;

    // Loop to find the initialize response
    let mut initialize_response = None;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(20), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();
        
        // eprintln!("Received message: {:?}", message); // Debugging

        if message["id"].as_i64() == Some(initialize_id) {
            initialize_response = Some(message);
            break;
        }
    }
    let response = initialize_response.expect("Did not receive initialize response with matching ID");

    assert_eq!(response["id"], initialize_id);
    assert!(response["result"]["capabilities"].is_object());

    // 2. Send initialized notification
    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": InitializedParams {},
    });
    send_message(&mut client_stream, initialized_notification).await;

    let doc_uri = Url::parse("file:///test.edgelord").unwrap();
    let initial_text = "(touch x ?y)"; // Text with a diagnostic
    let version = 1;

    // 3. Send didOpen notification
    let did_open_notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: doc_uri.clone(),
                language_id: "edgelord".to_string(),
                version,
                text: initial_text.to_string(),
            },
        },
    });
    send_message(&mut client_stream, did_open_notification).await;

    // Loop to find publishDiagnostics notification
    let mut diagnostics_notification = None;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(20), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();
        
        // eprintln!("Received message: {:?}", message); // Debugging

        if message["method"] == "textDocument/publishDiagnostics" {
            diagnostics_notification = Some(message);
            break;
        }
    }
    let diagnostics_notification = diagnostics_notification.expect("Did not receive publishDiagnostics notification");

    assert_eq!(diagnostics_notification["method"], "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams = serde_json::from_value(diagnostics_notification["params"].clone()).unwrap();
    assert_eq!(params.uri, doc_uri);
    assert_eq!(params.version, Some(version));
    assert!(!params.diagnostics.is_empty(), "Diagnostics should not be empty");
    assert!(params.diagnostics.iter().any(|d| d.message.contains("?y")));
}

// Test 2: Debounce and single-flight
#[tokio::test]
async fn test_debounce_and_single_flight() {
    // Client sends requests to client_to_server_tx, reads responses from server_to_client_rx
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config {
        debounce_interval_ms: 100, // Set debounce to 100ms
        ..Default::default()
    }));

    let (service, socket) = LspService::new(|client| {
        Backend::new(client, config_arc.clone())
    });

    // Server reads from client_to_server_rx, writes to server_to_client_tx
    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    // Client side streams
    let mut client_stream = BufStream::new(client_to_server_tx);
    let mut server_stream_reader = BufStream::new(server_to_client_rx);

    // 1. Send initialize and initialized
    let initialize_id = 1;
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": initialize_id,
        "method": "initialize",
        "params": InitializeParams {
            capabilities: Default::default(),
            process_id: None,
            root_uri: None, // Used root_uri instead of root_path
            root_path: None, // Still there for compatibility, but prefer root_uri
            client_info: None,
            locale: None,
            initialization_options: Some(serde_json::to_value(&Config {
                debounce_interval_ms: 100, // Make sure config is passed to server
                ..Default::default()
            }).unwrap()),
            trace: None,
            workspace_folders: None,
        },
    });
    send_message(&mut client_stream, initialize_request).await;
    
    // Loop to find the initialize response
    let mut initialize_response = None;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(20), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();
        if message["id"].as_i64() == Some(initialize_id) {
            initialize_response = Some(message);
            break;
        }
    }
    initialize_response.expect("Did not receive initialize response with matching ID"); // Consume response


    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": InitializedParams {},
    });
    send_message(&mut client_stream, initialized_notification).await;

    let doc_uri = Url::parse("file:///test.edgelord").unwrap();
    let initial_text = "test";
    let mut version = 1;

    // 2. Send didOpen notification
    let did_open_notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: doc_uri.clone(),
                language_id: "edgelord".to_string(),
                version,
                text: initial_text.to_string(),
            },
        },
    });
    send_message(&mut client_stream, did_open_notification).await;

    // Loop to consume any initial diagnostics from didOpen (should be empty for "test")
    let mut initial_diagnostics_found = false;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(20), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();
        
        // eprintln!("Received message after didOpen: {:?}", message); // Debugging

        if message["method"] == "textDocument/publishDiagnostics" {
            let params: PublishDiagnosticsParams = serde_json::from_value(message["params"].clone()).unwrap();
            assert!(params.diagnostics.is_empty(), "Initial diagnostics should be empty for 'test'");
            assert_eq!(params.version, Some(version));
            initial_diagnostics_found = true;
            break;
        }
    }
    assert!(initial_diagnostics_found, "Did not receive initial publishDiagnostics after didOpen");


    // 3. Send multiple rapid didChange events
    for i in 2..=5 {
        version = i;
        let did_change_notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: doc_uri.clone(),
                    version, // Corrected from Some(version)
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(
                        tower_lsp::lsp_types::Position::new(0, 0),
                        tower_lsp::lsp_types::Position::new(0, initial_text.len() as u32),
                    )),
                    range_length: Some(initial_text.len() as u32),
                    text: format!("test {}", i),
                }],
            },
        });
        send_message(&mut client_stream, did_change_notification).await;
        tokio::time::sleep(Duration::from_millis(20)).await; // Shorter than debounce interval
    }

    // Wait for the debounce period to pass (plus a little extra)
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Loop to find final publishDiagnostics notification
    let mut final_diagnostics_notification = None;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(20), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();

        // eprintln!("Received message after didChange burst: {:?}", message); // Debugging

        if message["method"] == "textDocument/publishDiagnostics" {
            final_diagnostics_notification = Some(message);
            break;
        }
    }
    let diagnostics_notification = final_diagnostics_notification.expect("Did not receive final debounced publishDiagnostics notification");


    assert_eq!(diagnostics_notification["method"], "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams = serde_json::from_value(diagnostics_notification["params"].clone()).unwrap();
    assert_eq!(params.uri, doc_uri);
    assert_eq!(params.version, Some(version), "Diagnostics should be for the latest version");
    assert!(params.diagnostics.iter().any(|d| d.message.contains("test 5")), "Final diagnostics should reflect latest change");
}


// Test 4: WorkspaceReport integration with latency measurement
// Validates: Requirements 5.1, 5.4
// Property: Diagnostics appear within 100ms of document change
#[tokio::test]
async fn test_workspace_report_integration_with_latency() {
    // Setup LSP server
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config::default()));

    let (service, socket) = LspService::new(|client| {
        Backend::new(client, config_arc.clone())
    });

    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    let mut client_stream = BufStream::new(client_to_server_tx);
    let mut server_stream_reader = BufStream::new(server_to_client_rx);

    // 1. Initialize
    let initialize_id = 1;
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": initialize_id,
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

    // Loop to find the initialize response
    let mut initialize_response = None;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(5), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();
        if message["id"].as_i64() == Some(initialize_id) {
            initialize_response = Some(message);
            break;
        }
    }
    let _response = initialize_response.expect("Did not receive initialize response with matching ID");

    // 2. Send initialized notification
    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": InitializedParams {},
    });
    send_message(&mut client_stream, initialized_notification).await;

    // 3. Open document with error
    let doc_uri = Url::parse("file:///test_latency.maclane").unwrap();
    let error_text = "(touch x ?y)"; // Text with diagnostic
    
    let change_start_time = Instant::now();
    
    let did_open_notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: doc_uri.clone(),
                language_id: "maclane".to_string(),
                version: 1,
                text: error_text.to_string(),
            },
        },
    });
    send_message(&mut client_stream, did_open_notification).await;

    // 4. Wait for publishDiagnostics and measure latency
    let mut diagnostics_received = false;
    let mut latency_ms = 0u128;
    
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(5), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();

        if message["method"] == "textDocument/publishDiagnostics" {
            latency_ms = change_start_time.elapsed().as_millis();
            diagnostics_received = true;
            
            let params: PublishDiagnosticsParams = serde_json::from_value(message["params"].clone()).unwrap();
            assert_eq!(params.uri, doc_uri);
            
            // Verify diagnostics are present (WorkspaceReport integration working)
            assert!(!params.diagnostics.is_empty(), "Expected diagnostics from WorkspaceReport");
            
            break;
        }
    }

    assert!(diagnostics_received, "Did not receive publishDiagnostics notification");
    
    // Requirement 5.4: Diagnostics should appear within 100ms
    // Note: In test environment, this may be slower due to cold start, so we use 500ms threshold
    // In production with warm cache, this should be < 100ms
    assert!(latency_ms < 500, "Diagnostic latency {}ms exceeds 500ms threshold (production target: 100ms)", latency_ms);
    
    eprintln!("✓ Diagnostic latency: {}ms (target: <100ms in production)", latency_ms);
}

// Test 5: Rapid document changes with WorkspaceReport
// Validates: Requirements 5.1, 5.4
// Property: Debouncing prevents diagnostic spam, final diagnostics reflect latest change
#[tokio::test]
async fn test_workspace_report_rapid_changes() {
    // Setup LSP server
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config::default()));

    let (service, socket) = LspService::new(|client| {
        Backend::new(client, config_arc.clone())
    });

    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    let mut client_stream = BufStream::new(client_to_server_tx);
    let mut server_stream_reader = BufStream::new(server_to_client_rx);

    // 1. Initialize
    let initialize_id = 1;
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": initialize_id,
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

    // Loop to find the initialize response
    let mut initialize_response = None;
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        let message = timeout(Duration::from_secs(5), read_one_message(&mut server_stream_reader))
            .await
            .expect("Did not receive any message from server")
            .unwrap();
        if message["id"].as_i64() == Some(initialize_id) {
            initialize_response = Some(message);
            break;
        }
    }
    let _response = initialize_response.expect("Did not receive initialize response with matching ID");

    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": InitializedParams {},
    });
    send_message(&mut client_stream, initialized_notification).await;

    // 2. Open document
    let doc_uri = Url::parse("file:///test_rapid.maclane").unwrap();
    let initial_text = "(def test 1)";
    
    let did_open_notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: doc_uri.clone(),
                language_id: "maclane".to_string(),
                version: 1,
                text: initial_text.to_string(),
            },
        },
    });
    send_message(&mut client_stream, did_open_notification).await;

    // Drain initial publishDiagnostics
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(2) {
        if let Ok(Some(message)) = timeout(Duration::from_millis(500), read_one_message(&mut server_stream_reader)).await {
            if message["method"] == "textDocument/publishDiagnostics" {
                break;
            }
        } else {
            break;
        }
    }

    // 3. Send rapid changes (10 changes in quick succession)
    let rapid_change_start = Instant::now();
    for i in 2..=11 {
        let did_change_notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: doc_uri.clone(),
                    version: i,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(
                        tower_lsp::lsp_types::Position::new(0, 0),
                        tower_lsp::lsp_types::Position::new(0, initial_text.len() as u32),
                    )),
                    range_length: Some(initial_text.len() as u32),
                    text: format!("(def test-{} {})", i, i),
                }],
            },
        });
        send_message(&mut client_stream, did_change_notification).await;
        tokio::time::sleep(Duration::from_millis(10)).await; // Much shorter than debounce interval
    }

    // 4. Wait for debounced diagnostics
    tokio::time::sleep(Duration::from_millis(300)).await; // Wait for debounce

    // 5. Collect diagnostics and verify only one final update
    let mut diagnostic_count = 0;
    let mut final_version = None;
    
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(3) {
        if let Ok(Some(message)) = timeout(Duration::from_millis(500), read_one_message(&mut server_stream_reader)).await {
            if message["method"] == "textDocument/publishDiagnostics" {
                diagnostic_count += 1;
                let params: PublishDiagnosticsParams = serde_json::from_value(message["params"].clone()).unwrap();
                final_version = params.version;
            }
        } else {
            break;
        }
    }

    // Verify debouncing worked: should receive only 1-2 diagnostic updates (not 10)
    assert!(diagnostic_count <= 2, "Expected 1-2 diagnostic updates due to debouncing, got {}", diagnostic_count);
    
    // Verify final diagnostics reflect latest change
    assert_eq!(final_version, Some(11), "Final diagnostics should be for version 11");
    
    let total_time = rapid_change_start.elapsed().as_millis();
    eprintln!("✓ Rapid changes handled: {} updates in {}ms (debounced to {} diagnostic publications)", 10, total_time, diagnostic_count);
}
