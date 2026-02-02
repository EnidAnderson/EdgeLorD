# EdgelorD LSP

![EdgeLorD Logo](EdgeLorD.png)

`edgelord-lsp` is an editor-agnostic LSP 3.17 proof assistant server for Comrade Lisp.

## MVP0 Status

Implemented now:
- LSP lifecycle: initialize/initialized/shutdown/exit
- Text sync: didOpen/didChange/didSave/didClose
- Parse diagnostics publishing
- `textDocument/selectionRange`
- `textDocument/documentSymbol`
- Basic `hover` and wired `codeAction` surface

## Local Dependencies

- `codeswitch` (sister directory): `../codeswitch`
- `new_surface_syntax` (Comrade parser/elaborator):
  `../clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule`

## Tests Added For MVP0

- `tests/selection_and_diagnostics.rs`
  - structural selection expansion shape (atom -> list -> form -> root)
  - parse diagnostic stability
- `tests/mvp0_utilities.rs`
  - UTF-16 position/offset roundtrips
  - deterministic incremental text-change application
  - deterministic top-level symbol extraction
  - selection-chain nesting validator behavior

## Run

```bash
cargo test
```

If your local rustup has no default toolchain configured, set one first:

```bash
rustup default stable
```
