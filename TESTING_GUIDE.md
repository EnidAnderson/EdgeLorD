# EdgeLorD LSP Testing Guide

## Test Harness Fix Summary

**Issue Found**: Integration tests were timing out because they were reading LSP notifications (like `window/logMessage`) instead of waiting for the actual response messages.

**Root Cause**: The LSP server sends notifications asynchronously, and the test harness was reading the first message it received, which was often a notification rather than the response to the request.

**Solution**: Modified test helpers to skip notifications and wait for messages with matching request IDs.

## Running Tests

### Smoke Tests (Minimal LSP Loop Verification)
```bash
cargo test --manifest-path EdgeLorD/Cargo.toml --test smoke_test
```

These tests verify:
- ✅ Server starts and responds to initialize
- ✅ Server handles initialized notification
- ✅ Server responds to shutdown

### Unit Tests (All Passing)
```bash
cargo test --manifest-path EdgeLorD/Cargo.toml --lib
```

Results:
- ✅ 66/66 lib tests pass
- ✅ 8/8 diagnostic publishing tests pass
- ✅ 21/21 span conversion tests pass

### Integration Tests (Need Fixing)
```bash
cargo test --manifest-path EdgeLorD/Cargo.toml --test integration_tests
```

**Status**: Need to apply the same notification-skipping pattern from smoke tests.

## Manual Testing with Helix

### Prerequisites
1. Build EdgeLorD LSP server:
   ```bash
   cargo build --manifest-path EdgeLorD/Cargo.toml --release
   ```

2. Find the binary path:
   ```bash
   ls EdgeLorD/target/release/edgelord-lsp
   ```

### Helix Configuration

Create or edit `~/.config/helix/languages.toml`:

```toml
# EdgeLorD / MacLane Language Support
[[language]]
name = "maclane"
scope = "source.maclane"
injection-regex = "maclane"
file-types = ["maclane", "ml", "edgelord"]
comment-token = "--"
indent = { tab-width = 2, unit = "  " }

[language-server.edgelord-lsp]
command = "/absolute/path/to/EdgeLorD/target/release/edgelord-lsp"
args = []

[[language]]
name = "maclane"
language-servers = ["edgelord-lsp"]
```

**Important**: Replace `/absolute/path/to/` with your actual path.

### Testing Steps

1. **Create a test file**:
   ```bash
   echo "(touch x ?y)" > test.maclane
   ```

2. **Open in Helix**:
   ```bash
   hx test.maclane
   ```

3. **Verify diagnostics appear**:
   - You should see a diagnostic about `?y` (hole syntax)
   - Diagnostics should appear within ~250ms (debounce interval)

4. **Test rapid changes**:
   - Type quickly and verify diagnostics update
   - Only final state should trigger diagnostic computation (debouncing)

5. **Check LSP server logs**:
   - Helix logs LSP communication to stderr
   - Run with: `RUST_LOG=debug hx test.maclane 2> lsp.log`
   - Check `lsp.log` for diagnostic messages

### Expected Behavior

✅ **Working**:
- Diagnostics appear for syntax errors
- Diagnostics appear for type errors
- Diagnostics update on file changes
- Debouncing prevents spam during rapid typing

❌ **Not Yet Implemented**:
- Hover information (DB-7 rename impact)
- Code actions
- Go to definition
- Document symbols

## Manual Testing with VS Code (Future)

### Prerequisites
1. Install VS Code extension development tools
2. Create extension manifest (`package.json`)
3. Configure language client

### Extension Structure
```
edgelord-vscode/
├── package.json          # Extension manifest
├── src/
│   └── extension.ts      # Language client setup
└── syntaxes/
    └── maclane.tmLanguage.json  # Syntax highlighting
```

### VS Code Configuration

`package.json`:
```json
{
  "name": "edgelord-lsp",
  "displayName": "EdgeLorD Language Support",
  "description": "LSP support for MacLane proof assistant",
  "version": "0.1.0",
  "engines": {
    "vscode": "^1.75.0"
  },
  "activationEvents": [
    "onLanguage:maclane"
  ],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [{
      "id": "maclane",
      "extensions": [".maclane", ".ml"],
      "configuration": "./language-configuration.json"
    }],
    "configuration": {
      "type": "object",
      "title": "EdgeLorD",
      "properties": {
        "edgelord.serverPath": {
          "type": "string",
          "default": "edgelord-lsp",
          "description": "Path to EdgeLorD LSP server"
        }
      }
    }
  }
}
```

`src/extension.ts`:
```typescript
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    const serverPath = vscode.workspace.getConfiguration('edgelord').get<string>('serverPath') || 'edgelord-lsp';
    
    const serverOptions: ServerOptions = {
        command: serverPath,
        args: []
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'maclane' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.maclane')
        }
    };

    client = new LanguageClient(
        'edgelord-lsp',
        'EdgeLorD Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
```

### Testing in VS Code

1. **Install extension**:
   ```bash
   npm install
   npm run compile
   code --install-extension .
   ```

2. **Open test file**:
   ```bash
   code test.maclane
   ```

3. **Verify features**:
   - ✅ Diagnostics appear in Problems panel
   - ✅ Inline error squiggles
   - ✅ Hover information (when implemented)
   - ✅ Code actions (when implemented)

## Performance Testing

### Latency Measurement

Create a test file with known errors:
```maclane
(def test (hole x))
(def test2 (hole y))
(def test3 (hole z))
```

Measure time from file save to diagnostic appearance:
```bash
time hx test.maclane
# Edit and save
# Note time until diagnostics appear
```

**Target**: <100ms for warm cache, <500ms for cold cache

### Cache Hit Rate Measurement

Enable cache statistics logging:
```bash
RUST_LOG=edgelord_lsp::caching=debug hx test.maclane 2> cache.log
grep "cache hit" cache.log | wc -l
grep "cache miss" cache.log | wc -l
```

**Target**: >80% hit rate on typical edits

## Troubleshooting

### Server Not Starting
- Check binary exists: `ls EdgeLorD/target/release/edgelord-lsp`
- Check permissions: `chmod +x EdgeLorD/target/release/edgelord-lsp`
- Run manually: `EdgeLorD/target/release/edgelord-lsp`

### No Diagnostics Appearing
- Check LSP logs for errors
- Verify file extension is recognized (`.maclane`, `.ml`)
- Check debounce interval (default 250ms)

### Diagnostics Too Slow
- Check cache is enabled: `EDGELORD_DISABLE_CACHES` should not be set
- Verify warm cache hits: check logs for "cache hit"
- Consider reducing debounce interval

### Test Harness Issues
- Ensure notifications are skipped when waiting for responses
- Use `read_one_message()` helper that handles notifications
- Add logging to debug message flow

## Next Steps

1. **Fix integration tests** - Apply notification-skipping pattern
2. **Add latency measurement tests** - Verify <100ms target
3. **Create VS Code extension** - Full-featured editor support
4. **Benchmark cache performance** - Generate Phase C2 CSV evidence
5. **Add more LSP features** - Hover, code actions, go-to-definition

## References

- LSP Specification: https://microsoft.github.io/language-server-protocol/
- Helix LSP Configuration: https://docs.helix-editor.com/languages.html
- VS Code Extension API: https://code.visualstudio.com/api
- tower-lsp Documentation: https://docs.rs/tower-lsp/
