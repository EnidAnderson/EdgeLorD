# Benchmark Workspace Fixture

## Purpose

Minimal but realistic workspace for Phase C2 benchmarks (cache performance measurement).

## Structure

### Import Chain: A → B → C

```
A.mc (imports B)
└── B.mc (imports C)
    └── C.mc (contains parse error)
```

### Files

**C.mc**
- Contains: ZWJ sequence (👨‍👩‍👧‍👦), combining mark (é), surrogate pair (🦀)
- Deterministic error: undefined_symbol at line 5
- Purpose: Tests span conversion under unicode stress; generates stable diagnostic

**B.mc**
- Imports C
- No errors (if C compiles)
- Purpose: Middle of dependency chain; recompiles when C changes

**A.mc**
- Imports B
- No errors (if B compiles)
- Purpose: Head of chain; tests transitive invalidation when C changes

## Benchmark Scenarios

### Scenario 1: hot_edit
- Edit B 100 times (append whitespace)
- Expected: Phase 1 cache hits on repeated edits (same content_hash)

### Scenario 2: cross_file
- Repeat 30 cycles of:
  1. Edit C (error location changes) → invalidates B, workspace_snapshot changes
  2. Touch A (no-op: re-send same bytes) → DV increments, content_hash unchanged
     - Expected A outcome: miss:deps_changed (workspace changed)
  3. Edit A (content changes) → direct recompile
     - Expected A outcome: miss:content_changed

## Deterministic Diagnostic

**File**: C.mc, line 5
**Error**: undefined_symbol (família👨‍👩‍👧‍👦 is not defined)
**Span**: Starts at identifier, includes unicode characters
**Purpose**: Stable enough to verify diagnostics_count > 0; validates compilation ran

## Fixture Validity Checks

Before benchmark runs:
- [ ] All three files parse without crashing
- [ ] At least one diagnostic present (diagnostics_count >= 1)
- [ ] Import chain resolves (no missing import errors, assuming syntax is valid)
- [ ] Unicode characters present in C.mc (regex: /[👨‍👩‍👧‍👦é🦀]/)

## Unicode Test Coverage

1. **ZWJ Sequence** (👨‍👩‍👧‍👦): Multi-codepoint emoji family
   - Tests: UTF-16 surrogate pairs, zero-width joiners
   - Span conversion must handle correctly

2. **Combining Mark** (é = e + \u{0301}): Separate base + combining character
   - Tests: Multi-codepoint normalization
   - Span conversion must not collapse combining marks

3. **Surrogate Pair** (🦀): Single codepoint outside BMP
   - Tests: UTF-16 surrogate handling
   - Span conversion must not split into invalid halves

All three are in the identifier `família👨‍👩‍👧‍👦`, exercising span boundaries through unicode.

## Future Enhancements

- Add a 4th file D that creates a cycle (A→B→D, D→A) to test cycle detection
- Add file with very long content to stress memory tracking (bytes_open_docs)
- Add cross-file diagnostic (e.g., type error in A that references B's exports) for richer invalidation patterns
