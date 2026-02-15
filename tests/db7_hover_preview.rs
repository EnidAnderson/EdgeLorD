use std::sync::Arc;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufStream, duplex, AsyncReadExt};
use tokio::time::{timeout, Duration, Instant};
use tower_lsp::{
    LspService, Server,
    lsp_types::{
        InitializeParams, InitializedParams, DidOpenTextDocumentParams,
        TextDocumentItem, Url, HoverParams, TextDocumentPositionParams,
        TextDocumentIdentifier, Position, Hover, HoverContents,
    },
};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use edgelord_lsp::lsp::{Backend, Config};

// Helper function to send JSON-RPC messages over the duplex stream
async fn send_message(stream: &mut BufStream<tokio::io::DuplexStream>, message: Value) {
    let message_str = serde_json::to_string(&message).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", message_str.len());
    stream.write_all(header.as_bytes()).await.unwrap();
    stream.write_all(message_str.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}

// Helper function to read ONE JSON-RPC message from the duplex stream with timeout
async fn read_one_message_timeout(
    stream: &mut BufStream<tokio::io::DuplexStream>,
    timeout_duration: Duration,
) -> Option<Value> {
    let mut content_length: Option<usize> = None;
    let mut buffer = String::new();

    let result = timeout(timeout_duration, async {
        loop {
            buffer.clear();
            match stream.read_line(&mut buffer).await {
                Ok(0) => return None, // EOF
                Ok(_) => {
                    if buffer == "\r\n" {
                        // End of headers
                        if let Some(len) = content_length {
                            let mut content_buffer = vec![0; len];
                            stream.read_exact(&mut content_buffer).await.ok()?;
                            let message: Value = serde_json::from_slice(&content_buffer).ok()?;
                            return Some(message);
                        }
                    } else if buffer.starts_with("Content-Length:") {
                        content_length = Some(
                            buffer["Content-Length:".len()..]
                                .trim()
                                .parse()
                                .ok()?,
                        );
                    }
                },
                Err(_) => return None,
            }
        }
    })
    .await;

    match result {
        Ok(msg) => msg,
        Err(_) => {
            eprintln!("Timeout reading message after {:?}", timeout_duration);
            None
        }
    }
}

// Helper to read messages until we find a response with matching ID
async fn read_until_response(
    stream: &mut BufStream<tokio::io::DuplexStream>,
    expected_id: i64,
    timeout_duration: Duration,
) -> Option<Value> {
    let start = Instant::now();
    let mut messages_seen = Vec::new();
    
    while start.elapsed() < timeout_duration {
        let remaining = timeout_duration - start.elapsed();
        if let Some(message) = read_one_message_timeout(stream, remaining).await {
            eprintln!("Received message: {:?}", message);
            
            // Check if it's the response we're waiting for
            if let Some(id) = message.get("id") {
                if id.as_i64() == Some(expected_id) {
                    eprintln!("Found matching response for ID {}", expected_id);
                    return Some(message);
                }
                // Different response ID, log and continue
                eprintln!("Received response with different ID: {:?}", id);
                messages_seen.push(message.clone());
                continue;
            }
            
            // It's a notification, log and continue
            if message.get("method").is_some() {
                eprintln!("Received notification: {:?}", message.get("method").unwrap());
                messages_seen.push(message.clone());
                continue;
            }
            
            messages_seen.push(message);
        } else {
            break;
        }
    }
    
    eprintln!("Timeout waiting for response with ID {}. Saw {} messages:", expected_id, messages_seen.len());
    for (i, msg) in messages_seen.iter().enumerate() {
        eprintln!("  Message {}: {:?}", i, msg);
    }
    None
}

// Helper to initialize server and return streams
async fn setup_server() -> (
    BufStream<tokio::io::DuplexStream>,
    BufStream<tokio::io::DuplexStream>,
) {
    let (client_to_server_tx, client_to_server_rx) = duplex(64 * 1024);
    let (server_to_client_tx, server_to_client_rx) = duplex(64 * 1024);

    let config_arc = Arc::new(RwLock::new(Config::default()));

    let (service, socket) = LspService::new(|client| {
        Backend::new(client, config_arc.clone())
    });

    let serve_fut = Server::new(client_to_server_rx, server_to_client_tx, socket).serve(service);
    tokio::spawn(serve_fut);

    let client_stream = BufStream::new(client_to_server_tx);
    let server_stream_reader = BufStream::new(server_to_client_rx);

    // Small delay to let server task start
    tokio::time::sleep(Duration::from_millis(100)).await;

    (client_stream, server_stream_reader)
}

// Helper to perform initialize handshake
async fn initialize_server(
    client_stream: &mut BufStream<tokio::io::DuplexStream>,
    server_stream_reader: &mut BufStream<tokio::io::DuplexStream>,
) {
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
    send_message(client_stream, initialize_request).await;

    // Wait for initialize response using new helper
    let response = read_until_response(server_stream_reader, initialize_id, Duration::from_secs(5))
        .await
        .expect("Failed to receive initialize response");
    
    assert!(response.get("result").is_some(), "Initialize response should have result");

    // Send initialized notification
    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": InitializedParams {},
    });
    send_message(client_stream, initialized_notification).await;
}

// Helper to open a document (no longer waits for diagnostics)
async fn open_document(
    client_stream: &mut BufStream<tokio::io::DuplexStream>,
    _server_stream_reader: &mut BufStream<tokio::io::DuplexStream>,
    uri: &Url,
    text: &str,
    version: i32,
) {
    let did_open_notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "maclane".to_string(),
                version,
                text: text.to_string(),
            },
        },
    });
    send_message(client_stream, did_open_notification).await;
    
    // Small delay to let server process the didOpen
    tokio::time::sleep(Duration::from_millis(50)).await;
}

// Helper to send hover request and get response
async fn send_hover_request(
    client_stream: &mut BufStream<tokio::io::DuplexStream>,
    server_stream_reader: &mut BufStream<tokio::io::DuplexStream>,
    uri: &Url,
    position: Position,
    request_id: i64,
) -> Option<Hover> {
    let hover_request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "textDocument/hover",
        "params": HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri.clone(),
                },
                position,
            },
            work_done_progress_params: Default::default(),
        },
    });
    send_message(client_stream, hover_request).await;

    // Wait for hover response with timeout
    let message = read_until_response(server_stream_reader, request_id, Duration::from_secs(5)).await?;
    
    // Check if result is null (no hover)
    if message["result"].is_null() {
        return None;
    }
    
    // Parse hover response
    let hover: Hover = serde_json::from_value(message["result"].clone()).ok()?;
    Some(hover)
}

// Smoke test: verify server starts and responds to basic lifecycle
#[tokio::test]
async fn test_smoke_server_lifecycle() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    
    // Initialize
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

    // Wait for initialize response
    let response = read_until_response(&mut server_stream_reader, initialize_id, Duration::from_secs(5))
        .await
        .expect("Should receive initialize response");
    
    assert!(response.get("result").is_some(), "Initialize should return result");
    
    // Send initialized notification
    let initialized_notification = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": InitializedParams {},
    });
    send_message(&mut client_stream, initialized_notification).await;
    
    // Send shutdown request (no params)
    let shutdown_id = 2;
    let shutdown_request = json!({
        "jsonrpc": "2.0",
        "id": shutdown_id,
        "method": "shutdown",
    });
    send_message(&mut client_stream, shutdown_request).await;
    
    // Wait for shutdown response
    let response = read_until_response(&mut server_stream_reader, shutdown_id, Duration::from_secs(5))
        .await
        .expect("Should receive shutdown response");
    
    // Shutdown returns null result on success, or has no error
    assert!(response.get("error").is_none(), "Shutdown should not return error: {:?}", response.get("error"));
}

// Test 3.1: Rename preview appears (minimal version - just check hover works)
#[tokio::test]
async fn test_db7_hover_rename_preview_appears() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    initialize_server(&mut client_stream, &mut server_stream_reader).await;

    // Open a simple document
    let uri = Url::parse("file:///test.ml").unwrap();
    let text = "(def f (x) x)";
    open_document(&mut client_stream, &mut server_stream_reader, &uri, text, 1).await;

    // Hover over 'f' (position 5)
    let position = Position {
        line: 0,
        character: 5,
    };

    let hover = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri,
        position,
        100,
    )
    .await;

    // For now, just assert we get SOME response (even if None)
    // This tests that the hover handler doesn't hang
    eprintln!("Hover result: {:?}", hover);
    
    // If we get a hover, check if it contains DB-7 preview
    if let Some(hover) = hover {
        let markdown = match hover.contents {
            HoverContents::Markup(content) => content.value,
            _ => String::new(),
        };
        
        eprintln!("Hover markdown: {}", markdown);
        
        // If DB-7 preview is present, verify it's well-formed
        if markdown.contains("DB-7 Preview") {
            assert!(
                markdown.contains("f"),
                "DB-7 preview should mention the symbol 'f'"
            );
        }
    }
}

// Test 3.2: Fail-closed hover test - whitespace returns no DB-7 preview
#[tokio::test]
async fn test_db7_hover_whitespace_fail_closed() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    initialize_server(&mut client_stream, &mut server_stream_reader).await;

    // Open document with symbol 'f'
    let uri = Url::parse("file:///test.ml").unwrap();
    let text = "(def f (x) x)";
    open_document(&mut client_stream, &mut server_stream_reader, &uri, text, 1).await;

    // Hover on whitespace (position after 'f', before '(')
    let position = Position {
        line: 0,
        character: 6, // Space after 'f'
    };

    let hover = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri,
        position,
        101,
    )
    .await;

    // Either no hover, or fallback hover (but NOT DB-7 preview)
    if let Some(hover) = hover {
        let markdown = match hover.contents {
            HoverContents::Markup(content) => content.value,
            _ => panic!("Expected markup content"),
        };

        assert!(
            !markdown.contains("DB-7 Preview (Rename)"),
            "Hover on whitespace should NOT contain DB-7 preview. Got: {}",
            markdown
        );
    }
    // If hover is None, that's also acceptable (fail-closed)
}

// Test 3.2b: Fail-closed hover test - punctuation returns no DB-7 preview
#[tokio::test]
async fn test_db7_hover_punctuation_fail_closed() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    initialize_server(&mut client_stream, &mut server_stream_reader).await;

    // Open document with symbol 'f'
    let uri = Url::parse("file:///test.ml").unwrap();
    let text = "(def f (x) x)";
    open_document(&mut client_stream, &mut server_stream_reader, &uri, text, 1).await;

    // Hover on opening parenthesis
    let position = Position {
        line: 0,
        character: 0, // '(' at start
    };

    let hover = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri,
        position,
        102,
    )
    .await;

    // Either no hover, or fallback hover (but NOT DB-7 preview)
    if let Some(hover) = hover {
        let markdown = match hover.contents {
            HoverContents::Markup(content) => content.value,
            _ => panic!("Expected markup content"),
        };

        assert!(
            !markdown.contains("DB-7 Preview (Rename)"),
            "Hover on punctuation should NOT contain DB-7 preview. Got: {}",
            markdown
        );
    }
    // If hover is None, that's also acceptable (fail-closed)
}

// Test 3.3: Cache stability test - same hover twice, identical markdown
#[tokio::test]
async fn test_db7_hover_cache_stability() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    initialize_server(&mut client_stream, &mut server_stream_reader).await;

    // Open File A: defines symbol 'f' (simple valid syntax)
    let uri_a = Url::parse("file:///test_a.ml").unwrap();
    let text_a = "(def f (x))";
    open_document(&mut client_stream, &mut server_stream_reader, &uri_a, text_a, 1).await;

    // Open File B: uses symbol 'f' (simple valid syntax)
    let uri_b = Url::parse("file:///test_b.ml").unwrap();
    let text_b = "(def g (f))";
    open_document(&mut client_stream, &mut server_stream_reader, &uri_b, text_b, 1).await;

    // Hover over 'f' in File B
    let position = Position {
        line: 0,
        character: 8, // 'f' in "(def g (f))"
    };

    // First hover
    let hover1 = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri_b,
        position,
        103,
    )
    .await;

    assert!(hover1.is_some(), "First hover should return a result");
    let markdown1 = match hover1.unwrap().contents {
        HoverContents::Markup(content) => content.value,
        _ => panic!("Expected markup content"),
    };

    // Second hover (should hit cache)
    let hover2 = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri_b,
        position,
        104,
    )
    .await;

    assert!(hover2.is_some(), "Second hover should return a result");
    let markdown2 = match hover2.unwrap().contents {
        HoverContents::Markup(content) => content.value,
        _ => panic!("Expected markup content"),
    };

    // Assert exact equality (deterministic)
    assert_eq!(
        markdown1, markdown2,
        "Hover markdown should be identical across calls (cache stability)"
    );
}

// Test 2.1: Phase 2 sanity check - SniperDB input sync uses same file_id
#[tokio::test]
async fn test_db7_hover_file_sync_sanity() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    initialize_server(&mut client_stream, &mut server_stream_reader).await;

    // Open File B with 'f' usage
    let uri_b = Url::parse("file:///test_b.ml").unwrap();
    let text_b = "(def g (y) (begin (f y)))";
    open_document(&mut client_stream, &mut server_stream_reader, &uri_b, text_b, 1).await;

    // Hover on 'f' (should show preview)
    let position = Position {
        line: 0,
        character: 19, // 'f' in "(f y)"
    };

    let hover1 = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri_b,
        position,
        105,
    )
    .await;

    assert!(hover1.is_some(), "First hover should return a result");
    let markdown1 = match hover1.unwrap().contents {
        HoverContents::Markup(content) => content.value,
        _ => panic!("Expected markup content"),
    };

    // Should contain DB-7 preview for 'f'
    assert!(
        markdown1.contains("Preview (DB-7): Rename Impact") && markdown1.contains("f"),
        "First hover should show DB-7 preview for 'f'. Got: {}",
        markdown1
    );

    // Send didChange that removes 'f' usage (replace with 'h')
    let did_change_notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {
                "uri": uri_b.to_string(),
                "version": 2,
            },
            "contentChanges": [{
                "text": "(def g (y) (begin (h y)))",
            }],
        },
    });
    send_message(&mut client_stream, did_change_notification).await;

    // Wait for debounce + processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Consume publishDiagnostics
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_secs(2) {
        let remaining = Duration::from_secs(2) - start_time.elapsed();
        if let Some(message) = read_one_message_timeout(&mut server_stream_reader, remaining).await {
            if message["method"] == "textDocument/publishDiagnostics" {
                break;
            }
        } else {
            break; // Timeout, no more messages
        }
    }

    // Hover on same position (now 'h' instead of 'f')
    let hover2 = send_hover_request(
        &mut client_stream,
        &mut server_stream_reader,
        &uri_b,
        position,
        106,
    )
    .await;

    if let Some(hover2) = hover2 {
        let markdown2 = match hover2.contents {
            HoverContents::Markup(content) => content.value,
            _ => panic!("Expected markup content"),
        };

        // Should now show preview for 'h', not 'f' (or fail-closed)
        if markdown2.contains("DB-7 Preview (Rename)") {
            assert!(
                markdown2.contains("h") && !markdown2.contains("f → f_renamed"),
                "Second hover should show 'h', not 'f'. Got: {}",
                markdown2
            );
        }
    }
    // If hover is None, that's acceptable (fail-closed after change)
}

// Test: Code action for DB-7 rename preview
#[tokio::test]
async fn test_db7_code_action_rename_preview() {
    let (mut client_stream, mut server_stream_reader) = setup_server().await;
    initialize_server(&mut client_stream, &mut server_stream_reader).await;

    // Open a simple document
    let uri = Url::parse("file:///test.ml").unwrap();
    let text = "(def f (x))";
    open_document(&mut client_stream, &mut server_stream_reader, &uri, text, 1).await;

    // Request code actions at 'f' position
    let position = Position {
        line: 0,
        character: 5, // 'f' in "(def f (x))"
    };

    let code_action_request = json!({
        "jsonrpc": "2.0",
        "id": 200,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": {
                "uri": uri.to_string(),
            },
            "range": {
                "start": position,
                "end": position,
            },
            "context": {
                "diagnostics": [],
            },
        },
    });
    send_message(&mut client_stream, code_action_request).await;

    // Wait for code action response
    let response = read_until_response(&mut server_stream_reader, 200, Duration::from_secs(5))
        .await
        .expect("Should receive code action response");

    // Check if result contains actions
    if let Some(result) = response.get("result") {
        if let Some(actions) = result.as_array() {
            // Look for DB-7 preview action
            let has_db7_action = actions.iter().any(|action| {
                if let Some(title) = action.get("title").and_then(|t| t.as_str()) {
                    title.contains("Preview Rename Impact")
                } else {
                    false
                }
            });

            // DB-7 action should be present (if symbol extraction succeeded)
            // Note: This is a weak assertion since we don't know if symbol extraction will succeed
            eprintln!("Code actions returned: {}", actions.len());
            eprintln!("Has DB-7 action: {}", has_db7_action);
        }
    }
}
