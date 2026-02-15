# Multiple Diagnostics Collection Plan

**Date**: 2026-02-08
**Status**: Current limitation identified, solution proposed
**Priority**: Future enhancement (not blocking current LSP functionality)

## Current Behavior

The EdgeLorD LSP currently shows **one error at a time** instead of all errors in a file.

### Example
Given this file with 3 errors:
```lisp
(begin
  (def broken-ref undefined-symbol)  ;; Error 1
  (touch x y)                         ;; Error 2
  (def untouched 'oops)              ;; Error 3
)
```

**Current**: Only Error 1 is shown
**Desired**: All 3 errors shown simultaneously

## Root Cause

The Surface MacLane elaborator uses Rust's `?` operator for error handling, which causes **early return** on the first error:

### Location
`clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule/src/typed_elaborate.rs`

### Problem Code
```rust
// Line 131 and 183
for expr in exprs {
    forms.push(self.elaborate_form(expr)?);  // ❌ Returns on first error
}
```

The `?` operator propagates errors immediately, stopping elaboration at the first problem.

## Solution: Error Recovery

To collect multiple diagnostics, the elaborator needs **error recovery**:

### Approach 1: Collect Errors Instead of Returning
```rust
let mut forms = Vec::new();
let mut errors = Vec::new();

for expr in exprs {
    match self.elaborate_form(expr) {
        Ok(form) => forms.push(form),
        Err(e) => {
            errors.push(e);
            // Continue processing remaining forms
        }
    }
}

// Return all errors at once
if !errors.is_empty() {
    return Err(ElaborationError::Multiple(errors));
}
```

### Approach 2: Diagnostic Accumulator
```rust
pub struct Elaborator {
    // ... existing fields ...
    diagnostics: Vec<StructuredDiagnostic>,  // Accumulate errors
}

impl Elaborator {
    fn elaborate_form(&mut self, expr: &SExpr) -> Option<CoreForm> {
        match self.try_elaborate_form(expr) {
            Ok(form) => Some(form),
            Err(e) => {
                // Convert error to diagnostic and accumulate
                self.diagnostics.push(e.into_diagnostic());
                None  // Continue processing
            }
        }
    }
}
```

### Approach 3: Scope-Level Error Recovery
```rust
// Elaborate each scope independently
for scope in scopes {
    match elaborate_scope(scope) {
        Ok(forms) => all_forms.extend(forms),
        Err(errors) => {
            // Collect errors from this scope
            all_diagnostics.extend(errors);
            // Continue with next scope
        }
    }
}
```

## Implementation Strategy

### Phase 1: Diagnostic Accumulator (Recommended)
1. Add `diagnostics: Vec<StructuredDiagnostic>` field to `Elaborator`
2. Change error-prone methods to accumulate instead of return
3. Continue processing after errors
4. Return all diagnostics at end

### Phase 2: Scope-Aware Recovery
1. Isolate errors to specific scopes
2. Allow partial elaboration of valid scopes
3. Provide better error context (which scope failed)

### Phase 3: Intelligent Error Recovery
1. Insert placeholder forms for failed elaborations
2. Continue type checking with partial information
3. Provide cascading error detection (errors that depend on earlier errors)

## Benefits

### For Users
- **See all errors at once** - no need to fix one error to see the next
- **Better workflow** - fix multiple issues in one pass
- **Scope awareness** - understand which parts of code are affected

### For Robots (AI Agents)
- **Complete error context** - see all problems in one diagnostic pass
- **Better fix planning** - understand dependencies between errors
- **Faster iteration** - fix multiple issues without re-running LSP

## Challenges

### 1. Error Cascades
Some errors depend on earlier errors:
```lisp
(def x undefined)  ;; Error 1: undefined symbol
(def y x)          ;; Error 2: x has unknown type (cascades from Error 1)
```

**Solution**: Mark cascading errors with lower severity or "may be spurious" notes.

### 2. Partial Elaboration
If elaboration fails partway through, later code may reference incomplete state.

**Solution**: Use placeholder/error forms to allow continued processing.

### 3. Performance
Collecting all errors may be slower than failing fast.

**Solution**: Add timeout or error limit (e.g., stop after 100 errors).

## Current Workaround

For now, users can:
1. Fix the first error shown
2. Save the file (triggers re-elaboration)
3. See the next error
4. Repeat until all errors fixed

This is functional but not ideal for productivity.

## Priority

**Not blocking**: The LSP works correctly, just shows one error at a time.

**Future enhancement**: Should be implemented after:
- Task 16.3 complete (integration tests passing)
- Task 17 complete (checkpoint verification)
- Basic LSP functionality stable

## Related Work

### Rust Compiler
Rust's compiler collects multiple errors using similar techniques:
- Error recovery at statement boundaries
- Partial type inference with error types
- Cascading error detection

### TypeScript LSP
TypeScript shows all errors simultaneously:
- Continues parsing after syntax errors
- Performs partial type checking
- Marks cascading errors

## Implementation Files

### Files to Modify
1. **`typed_elaborate.rs`** - Add diagnostic accumulator
2. **`ops.rs`** - Already collects all diagnostics (no change needed)
3. **`lsp.rs`** - Already publishes all diagnostics (no change needed)

### Files Already Correct
- ✅ `sniper_db/src/ops.rs` - Collects all diagnostics from all scopes
- ✅ `EdgeLorD/src/lsp.rs` - Publishes all diagnostics to LSP client
- ✅ `EdgeLorD/src/span_conversion.rs` - Handles multiple spans correctly

**The issue is only in the elaborator's error handling strategy.**

## Next Steps

1. **Document current behavior** ✅ (this document)
2. **Complete current tasks** (16.3, 17)
3. **Design error recovery strategy** (choose Approach 1, 2, or 3)
4. **Implement diagnostic accumulator**
5. **Test with multi-error files**
6. **Verify cascading error handling**

## Success Criteria

- [ ] All errors in a file shown simultaneously
- [ ] Errors are scope-aware (show which scope failed)
- [ ] Cascading errors marked appropriately
- [ ] Performance acceptable (<500ms for typical files)
- [ ] Error messages remain clear and actionable

## References

- **Elaborator**: `clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule/src/typed_elaborate.rs`
- **Diagnostic collection**: `clean_kernel/crates/sniper_db/src/ops.rs` lines 1212-1274
- **LSP publishing**: `EdgeLorD/src/lsp.rs`
- **Test file**: `EdgeLorD/test_examples/simple_error.maclane`
