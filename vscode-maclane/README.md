# Mac Lane Language Support for VS Code

Language support for `.maclane` files — the surface syntax for the Mac Lane rewriting-based proof engine.

## Features

- **Syntax highlighting** via TextMate grammar (works immediately)
- **Semantic tokens** via EdgeLorD LSP (richer, context-aware highlighting)
- **Hover** — coherence info, DB-7 lookup, proof state
- **Document symbols** — top-level definitions, rules, touches
- **Code actions** — Loogle search, DB-7 lookup
- **Diagnostics** — parse and elaboration errors
- **Commands** — show goals, restart server

## Setup

1. Build the EdgeLorD LSP server:
   ```bash
   cd EdgeLorD && cargo build --release
   ```

2. Either:
   - Set `maclane.server.path` in VS Code settings to the built binary path, or
   - Add the `target/release` directory to your `PATH`

3. Open a `.maclane` file — the extension activates automatically.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `maclane.server.path` | `""` | Path to `edgelord-lsp` binary |
| `maclane.server.extraArgs` | `[]` | Extra CLI arguments for the server |
| `maclane.trace.server` | `"off"` | LSP trace level (`off`, `messages`, `verbose`) |

## Without the LSP server

The extension provides baseline TextMate highlighting even without the LSP server running. You'll get:
- Keyword highlighting for `def`, `rule`, `touch`, `sugar`, `use`, `in`, etc.
- String and comment highlighting
- Number literals
- Pattern variable highlighting (`?x`, `?*xs`)
- Module path highlighting (`Module::Path`)

The LSP server adds semantic analysis on top of this baseline.
