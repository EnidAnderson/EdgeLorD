# Manual Testing Guide - Diagnostic Update Fix

**Purpose**: Verify that the diagnostic update bug fix works correctly in practice.

## Prerequisites

1. **Build the LSP server**:
   ```bash
   cargo build --release --manifest-path EdgeLorD/Cargo.toml --bin edgelord-lsp
   ```

2. **Verify Helix configuration**:
   ```bash
   # Check that languages.toml is configured
   cat ~/.config/helix/languages.toml | grep -A 10 "maclane"
   ```

3. **Ensure LSP server is in PATH or update config**:
   ```bash
   # Option 1: Add to PATH
   export PATH="$PATH:$(pwd)/EdgeLorD/target/release"
   
   # Option 2: Update languages.toml with absolute path
   # command = "/absolute/path/to/EdgeLorD/target/release/edgelord-lsp"
   ```

## Test Cases

### Test 1: Initial Load (Baseline)
**Purpose**: Verify diagnostics appear on initial file load

**Steps**:
1. Open test file:
   ```bash
   hx EdgeLorD/test_examples/simple_error.maclane
   ```

2. **Expected**: Diagnostics appear immediately
   - Error message visible in status bar
   - Error highlighted in editor
   - Diagnostic details in diagnostics panel

**Status**: ⏳ To be tested

---

### Test 2: Edit File (THE BUG FIX)
**Purpose**: Verify diagnostics update after edits (this was broken before)

**Steps**:
1. Open test file:
   ```bash
   hx EdgeLorD/test_examples/simple_error.maclane
   ```

2. Make a simple edit:
   - Add a space at the end of a line
   - Or add a newline

3. Wait 250ms (debounce delay)

4. **Expected**: Diagnostics still visible
   - Error message still in status bar
   - Error still highlighted
   - Diagnostics panel still shows errors

5. **Before fix**: Diagnostics would disappear ❌
6. **After fix**: Diagnostics should remain ✅

**Status**: ⏳ To be tested

---

### Test 3: Multiple Edits
**Purpose**: Verify diagnostics update correctly after multiple edits

**Steps**:
1. Open test file
2. Make first edit (add space)
3. Wait 250ms
4. Verify diagnostics visible
5. Make second edit (add another space)
6. Wait 250ms
7. Verify diagnostics still visible

**Expected**: Diagnostics update after each edit

**Status**: ⏳ To be tested

---

### Test 4: Fix Error
**Purpose**: Verify diagnostics disappear when error is fixed

**Steps**:
1. Open test file with error
2. Note the error message
3. Fix the error (e.g., add missing `touch` statement)
4. Wait 250ms

**Expected**: Diagnostic disappears
- No error in status bar
- No error highlighting
- Diagnostics panel empty or shows different errors

**Status**: ⏳ To be tested

---

### Test 5: Introduce New Error
**Purpose**: Verify new diagnostics appear when introducing errors

**Steps**:
1. Open test file with no errors (or fix existing errors)
2. Introduce a new error (e.g., reference undefined symbol)
3. Wait 250ms

**Expected**: New diagnostic appears
- Error message in status bar
- Error highlighted in editor
- Diagnostic details in panel

**Status**: ⏳ To be tested

---

### Test 6: Rapid Edits (Debouncing)
**Purpose**: Verify debouncing works correctly

**Steps**:
1. Open test file
2. Make rapid edits (type quickly without pausing)
3. Stop typing
4. Wait 250ms

**Expected**: 
- Diagnostics don't update on every keystroke (would be too slow)
- Diagnostics update once after you stop typing
- No flickering or excessive updates

**Status**: ⏳ To be tested

---

### Test 7: Save File (Workaround)
**Purpose**: Verify the old workaround still works

**Steps**:
1. Open test file
2. Make edit
3. Save file (`:w` in Helix)

**Expected**: Diagnostics update on save
- This was the workaround before the fix
- Should still work after the fix

**Status**: ⏳ To be tested

---

### Test 8: Code Actions (Integration)
**Purpose**: Verify diagnostics work with code actions

**Steps**:
1. Open test file with error
2. Place cursor on symbol
3. Press `Space + a` to open code actions
4. Select "Preview Rename Impact"

**Expected**: Code action works correctly
- Shows rename impact analysis
- Diagnostics still visible after code action

**Status**: ⏳ To be tested

---

### Test 9: Performance (Large File)
**Purpose**: Verify no performance regression

**Steps**:
1. Open a large MacLane file (if available)
2. Make edits
3. Observe responsiveness

**Expected**: 
- Diagnostics update within 250ms
- No noticeable lag
- Editor remains responsive

**Status**: ⏳ To be tested

---

## Test Results Template

Copy this template to record your test results:

```markdown
## Test Results - [Date]

### Test 1: Initial Load
- Status: ✅ PASS / ❌ FAIL
- Notes: 

### Test 2: Edit File (THE BUG FIX)
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 3: Multiple Edits
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 4: Fix Error
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 5: Introduce New Error
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 6: Rapid Edits (Debouncing)
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 7: Save File (Workaround)
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 8: Code Actions (Integration)
- Status: ✅ PASS / ❌ FAIL
- Notes:

### Test 9: Performance (Large File)
- Status: ✅ PASS / ❌ FAIL
- Notes:

## Overall Assessment
- Critical bug fixed: ✅ YES / ❌ NO
- Ready for production: ✅ YES / ❌ NO
- Issues found: [List any issues]
```

## Debugging Tips

### If diagnostics don't appear at all:
1. Check LSP server is running:
   ```bash
   ps aux | grep edgelord-lsp
   ```

2. Check Helix logs:
   ```bash
   tail -f ~/.cache/helix/helix.log
   ```

3. Check LSP server logs (if configured)

### If diagnostics are incorrect:
1. Verify file syntax is correct
2. Check for parser errors
3. Verify SniperDB is working correctly

### If performance is poor:
1. Check CPU usage
2. Verify memoization is working
3. Check for excessive recomputation

## Success Criteria

The fix is successful if:
- ✅ Test 2 (Edit File) passes - this was the critical bug
- ✅ All other tests pass
- ✅ No performance regression
- ✅ No new bugs introduced

## Next Steps After Testing

1. **If all tests pass**:
   - Update task status to complete
   - Document test results
   - Consider the bug fixed

2. **If tests fail**:
   - Document failure details
   - Investigate root cause
   - Implement additional fixes
   - Re-test

3. **If performance issues**:
   - Profile the code
   - Optimize hot paths
   - Consider caching strategies

---

**Ready to test!** Follow the test cases above and record your results.
