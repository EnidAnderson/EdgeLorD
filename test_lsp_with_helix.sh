#!/bin/bash
# Script to test EdgeLorD LSP with Helix editor

set -e

echo "=== EdgeLorD LSP Testing with Helix ==="
echo ""

# Step 1: Build the LSP server
echo "Step 1: Building LSP server..."
cargo build --manifest-path EdgeLorD/Cargo.toml --bin edgelord-lsp
echo "✓ LSP server built at EdgeLorD/target/debug/edgelord-lsp"
echo ""

# Step 2: Check Helix config
echo "Step 2: Checking Helix configuration..."
if [ -f ~/.config/helix/languages.toml ]; then
    echo "⚠ You already have ~/.config/helix/languages.toml"
    echo "  Please manually add the content from EdgeLorD/.helix/languages.toml"
    echo "  Or backup your existing config and copy:"
    echo "    cp ~/.config/helix/languages.toml ~/.config/helix/languages.toml.backup"
    echo "    cp EdgeLorD/.helix/languages.toml ~/.config/helix/languages.toml"
else
    echo "Creating Helix config directory..."
    mkdir -p ~/.config/helix
    cp EdgeLorD/.helix/languages.toml ~/.config/helix/languages.toml
    echo "✓ Helix config installed at ~/.config/helix/languages.toml"
fi
echo ""

# Step 3: Show test file location
echo "Step 3: Test file ready at EdgeLorD/test_examples/simple_error.maclane"
echo ""

# Step 4: Instructions
echo "=== Next Steps ==="
echo ""
echo "1. Open the test file in Helix:"
echo "   cd EdgeLorD && hx test_examples/simple_error.maclane"
echo ""
echo "2. What to look for:"
echo "   - Red squiggles/highlights on error lines"
echo "   - Press 'Space + d' to see diagnostics panel"
echo "   - Diagnostics should appear within ~250ms"
echo ""
echo "3. Test interactive editing:"
echo "   - Add new errors (undefined variables)"
echo "   - Fix existing errors"
echo "   - Type rapidly to test debouncing"
echo ""
echo "4. Optional: Run with debug logging:"
echo "   RUST_LOG=debug hx test_examples/simple_error.maclane 2> lsp_debug.log"
echo ""
echo "=== Expected Errors in Test File ==="
echo "   Line 10: (def broken-ref undefined-symbol) - undefined symbol"
echo "   Line 13: (touch x y) - invalid arity for touch (expected 1, found 2)"
echo ""
