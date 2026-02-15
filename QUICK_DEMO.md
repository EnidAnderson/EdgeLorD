# Quick Code Actions Demo - 30 Seconds

## Setup
```bash
cd EdgeLorD && hx test_examples/code_action_demo.maclane
```

## Steps
1. **Cursor** on `base-type` (line 11)
2. **Press** `Space + a` (code actions)
3. **Select** "Preview Rename Impact"
4. **See** SniperDB's semantic analysis! ✨

## What You'll See
```
Rename Impact Analysis: base-type → base-type_renamed

✓ Scope Analysis: 4 references in scope_0
✓ Dependencies: derived-type, helper-fn, another-use
✓ Indirect Impact: main-fn (via helper-fn)
✓ Cost: LOW (5 direct edits)
✓ Proofs: All preserved ✓
```

## Why It's Cool
- **Not just "find references"** - understands semantics!
- **Knows dependencies** - what depends on what
- **Predicts impact** - cost and complexity
- **Checks proofs** - will they still work?
- **Fast** - memoized by SniperDB

## Try Different Symbols
- `derived-type` (line 14) - fewer dependencies
- `helper-fn` (line 17) - used by main-fn
- `main-fn` (line 20) - leaf node, no dependents

Each shows different dependency patterns!

---

**This is SniperDB-powered semantic awareness in action!** 🚀
