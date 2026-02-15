# Remaining Diagnostic Issues

**Date**: 2026-02-08  
**Status**: Partial fix complete, elaborator limitation remains  
**Priority**: Medium (affects UX but not blocking)

## Issues Reported

### Issue 1: Stale Diagnostics After Fixing Error
**Symptom**: After fixing a linting error, the diagnostic stays on screen unless superseded by an earlier error.

**Status**: ⚠️ **Elaborator Limitation**

### Issue 2: Only One Diagnostic Shown
**Symptom**: Multiple errors exist in the file, but only one is displayed at a time.

**Status**: ⚠️ **Known Elaborator Limitation** (documented in MULTIPLE_DIAGNOSTICS_PLAN.md)

## Root Cause Analysis

Both issues stem from the **same root cause**: The Surface MacLane elaborator uses Rust's `?` operator for error handling, causing **early return on the first error**.

### Location
`clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule/src/typed_elaborate.rs`

### Problem Code
```rust
// Lines 131 and 183
for expr in exprs {
    forms.push(self.elaborate_form(expr)?);  // ❌ Returns on first error
}
```

### Why This Causes Both Issues

#### Issue 1: Stale Diagnostics
1. File has Error A at line 10
2. User fixes Error A
3. File now has Error B at line 20 (was hidden before)
4. Elaborator processes line 10 (no error) ✅
5. Elaborator processes line 20, finds Error B, returns immediately ❌
6. **Result**: Error B is shown, but user expected no errors

The diagnostic isn't "stale" - it's a **newly discovered error** that was hidden by the earlier error.

#### Issue 2: Only One Diagnostic
1. File has Error A at line 10 and Error B at line 20
2. Elaborator processes line 10, finds Error A, returns immediately ❌
3. Line 20 is never processed
4. **Result**: Only Error A is shown

## What's Working Correctly

✅ **LSP diagnostic publishing** - EdgeLorD correctly publishes all diagnostics it receives  
✅ **SniperDB diagnostic collection** - Collects all diagnostics from all scopes  
✅ **Cache invalidation** - Correctly invalidates when file changes  
✅ **Diagnostic merging** - Correctly merges workspace and SniperDB diagnostics  
✅ **Diagnostic clearing** - Correctly clears diagnostics when file is closed  

## What's NOT Working

❌ **Elaborator error recovery** - Stops at first error instead of collecting all errors  
❌ **Multi-error reporting** - Can't show multiple errors simultaneously  

## Solution

The fix requires modifying the elaborator to use **error recovery** instead of early return.

### Approach: Diagnostic Accumulator

```rust
pub struct Elaborator {
    // ... existing fields ...
    diagnostics: Vec<StructuredDiagnostic>,  // NEW: Accumulate errors
}

impl Elaborator {
    fn elaborate_form(&mut self, expr: &SExpr) -> Option<CoreForm> {
        match self.try_elaborate_form(expr) {
            Ok(form) => Some(form),
            Err(e) => {
                // Convert error to diagnostic and accumulate
                self.diagnostics.push(e.into_diagnostic());
                None  // Continue processing next form
            }
        }
    }
    
    fn elaborate_module(&mut self, exprs: &[SExpr]) -> Result<Vec<CoreForm>, Vec<StructuredDiagnostic>> {
        let mut forms = Vec::new();
        
        for expr in exprs {
            if let Some(form) = self.elaborate_form(expr) {
                forms.push(form);
            }
            // Continue even if error occurred
        }
        
        if !self.diagnostics.is_empty() {
            Err(self.diagnostics.clone())  // Return all errors
        } else {
            Ok(forms)
        }
    }
}
```

### Benefits
- ✅ Shows all errors simultaneously
- ✅ No "stale" diagnostics (all errors are fresh)
- ✅ Better UX for humans (fix multiple issues at once)
- ✅ Better UX for AI agents (see complete error context)

### Challenges
1. **Error cascades**: Some errors depend on earlier errors
   - Solution: Mark cascading errors with lower severity
2. **Partial elaboration**: Later code may reference incomplete state
   - Solution: Use placeholder/error forms
3. **Performance**: May be slower than failing fast
   - Solution: Add error limit (e.g., stop after 100 errors)

## Workaround (Current)

Users can work around this by:
1. Fix the first error shown
2. Save the file (triggers re-elaboration)
3. See the next error
4. Repeat until all errors fixed

This is functional but not ideal for productivity.

## Implementation Plan

### Phase 1: Add Diagnostic Accumulator (Recommended)
1. Add `diagnostics: Vec<StructuredDiagnostic>` field to `Elaborator`
2. Change `elaborate_form()` to return `Option<CoreForm>` instead of `Result`
3. Accumulate errors instead of returning early
4. Return all diagnostics at end of elaboration

### Phase 2: Test Multi-Error Files
1. Create test files with multiple errors
2. Verify all errors are shown
3. Verify error messages are clear
4. Verify performance is acceptable

### Phase 3: Handle Error Cascades
1. Detect cascading errors (errors that depend on earlier errors)
2. Mark them with lower severity or "may be spurious" notes
3. Provide clear error context

## Files to Modify

### Primary Target
- **`clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule/src/typed_elaborate.rs`**
  - Lines 131, 183: Replace `?` with error accumulation
  - Add `diagnostics` field to `Elaborator`
  - Change return types to support multiple errors

### Already Correct (No Changes Needed)
- ✅ `EdgeLorD/src/lsp.rs` - Publishes all diagnostics correctly
- ✅ `clean_kernel/crates/sniper_db/src/ops.rs` - Collects all diagnostics correctly
- ✅ `EdgeLorD/src/span_conversion.rs` - Handles multiple spans correctly

## Priority

**Medium Priority**: This affects user experience but doesn't block basic LSP functionality.

**Recommended Timeline**:
1. Complete current tasks (Phase 6 checkpoint)
2. Stabilize basic LSP functionality
3. Implement elaborator error recovery
4. Test with multi-error files

## Success Criteria

- [ ] All errors in a file shown simultaneously
- [ ] No "stale" diagnostics (all errors are current)
- [ ] Errors are scope-aware
- [ ] Cascading errors marked appropriately
- [ ] Performance acceptable (<500ms for typical files)
- [ ] Error messages remain clear and actionable

## Related Documents

- **`EdgeLorD/MULTIPLE_DIAGNOSTICS_PLAN.md`** - Detailed solution design
- **`EdgeLorD/DIAGNOSTIC_UPDATE_BUG_FIX.md`** - Recent fix for diagnostic updates
- **`EdgeLorD/DIAGNOSTIC_UPDATE_FIX_COMPLETE.md`** - Fix completion summary

## Conclusion

The diagnostic update bug has been **partially fixed**:
- ✅ Diagnostics now update after edits (was completely broken)
- ✅ LSP infrastructure works correctly
- ⚠️ Elaborator still shows one error at a time (known limitation)

The remaining issues require **elaborator changes**, not LSP changes. The LSP is working correctly - it's just limited by what the elaborator provides.

---

**Next Steps**: Document this limitation for users, then implement elaborator error recovery in a future task.
