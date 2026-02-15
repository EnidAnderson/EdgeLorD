# Code Actions Demo Guide - SniperDB Semantic Awareness

**Date**: 2026-02-08
**Feature**: DB-7 "Preview Rename Impact" Code Action
**Status**: ✅ Implemented and Ready to Demo

## What This Demonstrates

The **"Preview Rename Impact"** code action showcases SniperDB's semantic understanding:

1. **Scope-aware analysis**: Understands which symbols are in scope
2. **Dependency tracking**: Knows what depends on what
3. **Impact prediction**: Estimates cost and complexity of changes
4. **Proof preservation**: Checks if proofs remain valid after rename
5. **Incremental computation**: Uses SniperDB's memoization for fast results

## How to Try It

### Step 1: Open the Demo File
```bash
cd EdgeLorD && hx test_examples/code_action_demo.maclane
```

### Step 2: Position Cursor
Place your cursor on a symbol name, for example:
- Line 11: `base-type` (in the def statement)
- Line 14: `derived-type`
- Line 17: `helper-fn`

### Step 3: Trigger Code Actions
In Helix, press: **`Space + a`** (or `:code-action`)

You should see a menu with options like:
```
> Preview Rename Impact: base-type → base-type_renamed
> Preview Rename Impact (Detailed): base-type → base-type_renamed
```

### Step 4: Select the Action
Choose either the compact or detailed preview.

### Step 5: See the Magic! ✨
SniperDB will analyze the entire codebase and show:
- **All references** to the symbol
- **Scope information** (which scopes contain references)
- **Dependency graph** (what depends on this symbol)
- **Cost estimation** (how complex is this rename)
- **Proof status** (will proofs still be valid)

## What Makes This Special

### Traditional LSP (e.g., TypeScript)
```
Find References: base-type
  - Line 11: definition
  - Line 14: usage
  - Line 17: usage
  - Line 20: usage
  - Line 23: usage
```

**Just a list of locations** - no semantic understanding.

### SniperDB-Powered LSP (EdgeLorD)
```
Rename Impact Analysis: base-type → base-type_renamed

Scope Analysis:
  - Defined in: scope_0 (begin block)
  - Referenced in: 4 locations within scope_0
  
Dependency Graph:
  - derived-type depends on base-type
  - helper-fn depends on base-type
  - main-fn indirectly depends via helper-fn
  - another-use depends on base-type

Cost Estimation:
  - Direct edits: 5 locations
  - Indirect impacts: 1 (main-fn)
  - Proof obligations: 0 new constraints
  - Estimated complexity: LOW

Proof Preservation:
  - All existing proofs remain valid
  - No new proof obligations introduced
  - Safe to rename ✓
```

**Semantic understanding** - knows the structure, dependencies, and implications!

## Technical Details

### How It Works

1. **User triggers code action** on a symbol
2. **EdgeLorD LSP** extracts symbol at cursor position
3. **Builds RenameSymbol intent** with deterministic target name
4. **Calls SniperDB's `plan_report_query`** (DB-7 feature)
5. **SniperDB analyzes**:
   - Scope resolution
   - Dependency tracking
   - Cost estimation
   - Proof impact
6. **Returns structured report** with all findings
7. **EdgeLorD renders** as markdown in code action
8. **User sees** complete impact analysis

### Key SniperDB Features Used

- **`plan_report_query`**: Memoized, pure, deterministic planning
- **Scope tracking**: Knows which symbols are in which scopes
- **Dependency graph**: Tracks what depends on what
- **Cost estimation**: Predicts complexity of changes
- **Proof preservation**: Checks if proofs remain valid

### Configuration

The feature is enabled by default via:
```toml
[language-server.edgelord-lsp.config]
enableDb7HoverPreview = true
```

You can also configure:
- `db7PlaceholderSuffix = "_renamed"` - Suffix for preview renames
- `db7DebugMode = false` - Show internal details

## Other Code Actions Available

### 1. Tactic Actions
- **QuickFix**: Fix common errors automatically
- **Refactor**: Restructure code
- **Rewrite**: Apply rewrite rules
- **Explain**: Show explanation of term
- **Expand**: Inline definitions

### 2. Loogle Actions
- **Lemma suggestions**: Find relevant lemmas from library
- **Type-based search**: Find functions by type signature

### 3. DB-7 Actions (This Demo)
- **Preview Rename Impact (Compact)**: Quick overview
- **Preview Rename Impact (Detailed)**: Full analysis

## Why This Matters

### For Human Developers
- **Confidence**: Know the impact before making changes
- **Safety**: See if proofs will break
- **Understanding**: Learn the codebase structure
- **Efficiency**: Make informed decisions quickly

### For AI Agents
- **Planning**: Understand change implications before acting
- **Safety**: Avoid breaking changes
- **Learning**: Understand codebase structure
- **Efficiency**: Make better edit decisions

## Comparison with Other Tools

### VS Code (TypeScript)
- ✅ Find references
- ✅ Rename symbol
- ❌ No impact analysis
- ❌ No cost estimation
- ❌ No proof preservation check

### IntelliJ (Java)
- ✅ Find references
- ✅ Rename symbol
- ✅ Preview changes
- ❌ No semantic cost estimation
- ❌ No proof preservation

### EdgeLorD (MacLane)
- ✅ Find references
- ✅ Rename symbol
- ✅ Preview changes
- ✅ **Semantic cost estimation**
- ✅ **Proof preservation check**
- ✅ **Dependency graph analysis**
- ✅ **Scope-aware impact**

## Future Enhancements

### Planned Features
1. **Actual rename execution**: Not just preview, but apply the rename
2. **Multi-file analysis**: Analyze impact across multiple files
3. **Proof repair suggestions**: Suggest how to fix broken proofs
4. **Cascading rename**: Automatically rename dependent symbols
5. **Undo/redo support**: Safe experimentation with changes

### Advanced SniperDB Features
1. **Incremental updates**: Only recompute changed parts
2. **Parallel analysis**: Analyze multiple symbols simultaneously
3. **Historical tracking**: See how dependencies evolved
4. **Optimization suggestions**: Recommend better structures

## Troubleshooting

### Code Actions Don't Appear
1. **Check config**: Ensure `enableDb7HoverPreview = true`
2. **Rebuild LSP**: `cargo build --manifest-path EdgeLorD/Cargo.toml --bin edgelord-lsp`
3. **Restart Helix**: Close and reopen the editor
4. **Check cursor position**: Must be on a valid symbol

### Analysis Seems Slow
1. **First run**: SniperDB builds cache (may take longer)
2. **Subsequent runs**: Should be fast (memoized)
3. **Large files**: May take longer for complex analysis
4. **Check logs**: `RUST_LOG=debug hx ...` to see what's happening

### Preview Shows "No Impact"
1. **Symbol not used**: The symbol has no references
2. **Scope issue**: Symbol might be shadowed or out of scope
3. **Parse error**: File might have syntax errors

## Demo Script

### Quick Demo (2 minutes)
1. Open `code_action_demo.maclane`
2. Cursor on `base-type` (line 11)
3. Press `Space + a`
4. Select "Preview Rename Impact"
5. Show the analysis output
6. Explain: "SniperDB knows all dependencies!"

### Full Demo (5 minutes)
1. Show the demo file structure
2. Explain the symbol relationships
3. Trigger code action on `base-type`
4. Walk through the analysis:
   - Scope information
   - Dependency graph
   - Cost estimation
   - Proof preservation
5. Try on different symbols (`derived-type`, `helper-fn`)
6. Show how analysis differs for each
7. Explain SniperDB's semantic understanding

### Technical Deep Dive (10 minutes)
1. Show the code action implementation (`lsp.rs`)
2. Explain `plan_report_query` call
3. Show SniperDB's planning logic
4. Demonstrate memoization (run twice, second is instant)
5. Show configuration options
6. Discuss future enhancements

## Success Metrics

### What to Look For
- ✅ Code actions appear in menu
- ✅ Analysis completes quickly (<1 second)
- ✅ Results are accurate (all references found)
- ✅ Scope information is correct
- ✅ Dependency graph is complete
- ✅ Cost estimation is reasonable
- ✅ Proof status is accurate

### What Makes a Good Demo
- **Responsive**: Actions appear immediately
- **Accurate**: All dependencies found
- **Clear**: Analysis is easy to understand
- **Impressive**: Shows semantic understanding
- **Useful**: Provides actionable information

## Conclusion

The **"Preview Rename Impact"** code action is a perfect showcase of SniperDB's semantic awareness. It goes far beyond traditional "find references" to provide:

- **Scope-aware analysis**
- **Dependency tracking**
- **Cost estimation**
- **Proof preservation checking**
- **Incremental computation**

This is **world-class tooling** that demonstrates the power of having a semantic database backing your LSP!

---

**Ready to demo?** Open `code_action_demo.maclane` and try it out! 🚀
