use source_span::Span;
use tower_lsp::lsp_types::{Position, Range};

/// Converts a byte offset to an LSP Position (line, character), using UTF-16 code units.
///
/// This function is the canonical way to convert from internal byte offsets to LSP positions.
/// It handles UTF-16 surrogate pairs correctly, which is required by the LSP spec.
///
/// Returns None if the offset is out of bounds or falls in the middle of a multi-byte character.
pub fn offset_to_position(text: &str, offset: usize) -> Option<Position> {
    if offset > text.len() {
        return None;
    }
    
    // Check if offset is at a valid character boundary
    if !text.is_char_boundary(offset) {
        return None;
    }

    let mut line = 0;
    let mut last_line_start = 0;
    
    // Iterate over characters to find line number and start of current line
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            last_line_start = i + 1; // Start of next line
        }
    }

    // Now calculate the column in UTF-16 code units
    let col_str = &text[last_line_start..offset];
    let character = col_str.encode_utf16().count() as u32;

    Some(Position::new(line, character))
}

/// Converts an LSP Position (line, character) to a byte offset.
///
/// This is the inverse of `offset_to_position`. It interprets the character index as
/// a count of UTF-16 code units.
pub fn position_to_offset(text: &str, position: Position) -> Option<usize> {
    let mut current_line = 0;
    let mut current_offset = 0;
    let target_line = position.line;
    let target_char_utf16 = position.character;

    // Fast forward to the target line
    if target_line > 0 {
        let mut lines = text.split_inclusive('\n');
        for _ in 0..target_line {
            if let Some(line_str) = lines.next() {
                if !line_str.ends_with('\n') {
                    return None; 
                }
                current_offset += line_str.len();
                current_line += 1;
            } else {
                return None; // Line out of bounds
            }
        }
    }

    // Now scan characters in the target line to find the byte offset corresponding
    // to the UTF-16 character index.
    let remaining_text = &text[current_offset..];
    // Find limits of current line to prevent crossing into next line
    let line_len = remaining_text.find('\n').map(|i| i + 1).unwrap_or(remaining_text.len());
    let line_str = &remaining_text[..line_len];

    let mut char_count_utf16 = 0;
    for (i, c) in line_str.char_indices() {
        if char_count_utf16 == target_char_utf16 {
            return Some(current_offset + i);
        }
        char_count_utf16 += c.len_utf16() as u32;
    }

    // Handle end of line/file case where cursor is after last char
    if char_count_utf16 == target_char_utf16 {
        return Some(current_offset + line_str.len()); // Allow position at EOF or EOL
    } else if text.ends_with('\n') && char_count_utf16 == target_char_utf16 + 1 {
         // Special case: cursor at EOL before newline char but logically on same line?? 
         // Actually LSP positions are 0-based between characters.
         // If we are here, we might have overshot or target char is beyond line length.
         // If target_char_utf16 matches length of line (excluding newline perhaps?), it might be valid.
         // But here we return None if we didn't match exactly.
         // Let's rely on exact match logic above.
         // If strict match fails, we check bounds.
         return None;
    }

    // If we're here, target position is past the end of the line
    None
}

/// Converts an internal byte Span to an LSP Range.
///
/// Uses `offset_to_position` for start and end, ensuring consistent UTF-16 handling.
pub fn byte_span_to_lsp_range(text: &str, span: Span) -> Option<Range> {
    let start = offset_to_position(text, span.start)?;
    let end = offset_to_position(text, span.end)?;
    Some(Range::new(start, end))
}

/// Converts a Span to an LSP Range with error handling.
///
/// This is the canonical API for converting SniperDB spans to LSP ranges.
/// Returns Result for better error reporting.
///
/// # Requirements
/// - Validates: Requirements 5.2, 6.1, 6.2, 6.3
///
/// # Errors
/// Returns an error if the span is invalid (out of bounds, negative positions, etc.)
pub fn span_to_lsp_range(span: &Span, source: &str) -> Result<Range, SpanConversionError> {
    // Validate span bounds
    if span.start > source.len() {
        return Err(SpanConversionError::OutOfBounds {
            offset: span.start,
            text_len: source.len(),
        });
    }
    if span.end > source.len() {
        return Err(SpanConversionError::OutOfBounds {
            offset: span.end,
            text_len: source.len(),
        });
    }
    if span.start > span.end {
        return Err(SpanConversionError::InvalidSpan {
            start: span.start,
            end: span.end,
        });
    }

    let start = offset_to_position(source, span.start)
        .ok_or(SpanConversionError::ConversionFailed {
            offset: span.start,
        })?;
    let end = offset_to_position(source, span.end)
        .ok_or(SpanConversionError::ConversionFailed {
            offset: span.end,
        })?;

    Ok(Range::new(start, end))
}

/// Converts a byte offset to a UTF-16 position.
///
/// This is an alias for `offset_to_position` with the naming convention from the design document.
///
/// # Requirements
/// - Validates: Requirements 5.2, 6.1, 6.2
pub fn byte_offset_to_utf16_position(source: &str, byte_offset: usize) -> Option<Position> {
    offset_to_position(source, byte_offset)
}

/// Error type for span conversion failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanConversionError {
    /// Span offset is out of bounds for the source text
    OutOfBounds { offset: usize, text_len: usize },
    /// Span has invalid bounds (start > end)
    InvalidSpan { start: usize, end: usize },
    /// Failed to convert offset to position
    ConversionFailed { offset: usize },
}

impl std::fmt::Display for SpanConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanConversionError::OutOfBounds { offset, text_len } => {
                write!(
                    f,
                    "Span offset {} is out of bounds (text length: {})",
                    offset, text_len
                )
            }
            SpanConversionError::InvalidSpan { start, end } => {
                write!(f, "Invalid span: start {} > end {}", start, end)
            }
            SpanConversionError::ConversionFailed { offset } => {
                write!(f, "Failed to convert offset {} to position", offset)
            }
        }
    }
}

impl std::error::Error for SpanConversionError {}


#[cfg(test)]
mod tests {
    use super::*;

    // --- Unit Tests: UTF-16 Correctness Test Matrix ---

    #[test]
    fn ascii_only() {
        let text = "def foo\n  bar";
        // 'b' in 'bar' is at offset 10 (8 for "def foo\n" + 2 spaces)
        let pos = offset_to_position(text, 10).unwrap();
        assert_eq!(pos, Position::new(1, 2));

        let offset = position_to_offset(text, Position::new(1, 2)).unwrap();
        assert_eq!(offset, 10);
    }

    #[test]
    fn multibyte_unicode_emoji() {
        let text = "hello 🦀 world"; 
        // 🦀 is 4 bytes, 2 UTF-16 code units (surrogate pair)
        // "hello " is 6 bytes.
        // "🦀" starts at 6.
        // " world" starts at 10.
        
        // Start of crab
        let pos = offset_to_position(text, 6).unwrap();
        assert_eq!(pos, Position::new(0, 6));

        // After crab (should advance by 2 chars in UTF-16)
        let pos = offset_to_position(text, 10).unwrap(); 
        assert_eq!(pos, Position::new(0, 8)); // 6 + 2 = 8

        // Round trip
        let off = position_to_offset(text, Position::new(0, 8)).unwrap();
        assert_eq!(off, 10);
    }

    #[test]
    fn multibyte_cjk() {
        let text = "你好"; 
        // Each is 3 bytes, 1 UTF-16 code unit
        
        let pos = offset_to_position(text, 3).unwrap(); // After 你
        assert_eq!(pos, Position::new(0, 1));
        
        let pos = offset_to_position(text, 6).unwrap(); // After 好
        assert_eq!(pos, Position::new(0, 2));
    }

    #[test]
    fn crlf_line_endings() {
        let text = "line1\r\nline2";
        // "line1\r\n" is 7 bytes
        
        let pos = offset_to_position(text, 7).unwrap(); // Start of line2
        assert_eq!(pos, Position::new(1, 0));

        let pos_r = offset_to_position(text, 6).unwrap();
        assert_eq!(pos_r, Position::new(0, 6));
    }

    #[test]
    fn empty_spans() {
        let text = "foo";
        let span = Span::new(1, 1); // Empty span at offset 1
        let range = byte_span_to_lsp_range(text, span).unwrap();
        assert_eq!(range.start, range.end);
        assert_eq!(range.start, Position::new(0, 1));
    }

    #[test]
    fn eof_positions() {
        let text = "foo";
        let pos = offset_to_position(text, 3).unwrap();
        assert_eq!(pos, Position::new(0, 3));
        
        let off = position_to_offset(text, Position::new(0, 3)).unwrap();
        assert_eq!(off, 3);
    }
    
    #[test]
    fn out_of_bounds() {
        let text = "foo";
        assert!(offset_to_position(text, 4).is_none());
        assert!(position_to_offset(text, Position::new(0, 4)).is_none());
        assert!(position_to_offset(text, Position::new(1, 0)).is_none());
    }

    #[test]
    fn span_to_lsp_range_valid() {
        let text = "hello world";
        let span = Span::new(0, 5);
        let range = span_to_lsp_range(&span, text).unwrap();
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 5));
    }

    #[test]
    fn span_to_lsp_range_out_of_bounds() {
        let text = "hello";
        let span = Span::new(0, 10);
        let result = span_to_lsp_range(&span, text);
        assert!(matches!(result, Err(SpanConversionError::OutOfBounds { .. })));
    }

    #[test]
    fn span_to_lsp_range_invalid_span() {
        let text = "hello";
        let span = Span::new(5, 2);
        let result = span_to_lsp_range(&span, text);
        assert!(matches!(result, Err(SpanConversionError::InvalidSpan { .. })));
    }

    #[test]
    fn byte_offset_to_utf16_position_alias() {
        let text = "hello 🦀";
        let pos = byte_offset_to_utf16_position(text, 6).unwrap();
        assert_eq!(pos, Position::new(0, 6));
    }

    #[test]
    fn error_display() {
        let err = SpanConversionError::OutOfBounds {
            offset: 10,
            text_len: 5,
        };
        assert_eq!(
            err.to_string(),
            "Span offset 10 is out of bounds (text length: 5)"
        );

        let err = SpanConversionError::InvalidSpan { start: 5, end: 2 };
        assert_eq!(err.to_string(), "Invalid span: start 5 > end 2");

        let err = SpanConversionError::ConversionFailed { offset: 10 };
        assert_eq!(err.to_string(), "Failed to convert offset 10 to position");
    }
    
    // Regression test: emoji + combining marks + multi-line
    #[test]
    fn regression_emoji_combining_marks_multiline() {
        // Test with emoji, combining marks (e + combining accent), and multiple lines
        // Note: Using explicit combining character (U+0301) not precomposed é
        let text = "line1: 🦀\nline2: e\u{0301}\nline3: 👨‍👩‍👧‍👦"; // Family emoji with ZWJ sequences
        
        // Line 0: "line1: 🦀\n" (7 + 4 + 1 = 12 bytes)
        // Line 1: "line2: e\u{0301}\n" (7 + 1 + 2 + 1 = 11 bytes, e + combining accent)
        // Line 2: "line3: 👨‍👩‍👧‍👦" (7 + 25 bytes for family emoji)
        
        // Test crab emoji on line 0
        let crab_start = 7;
        let pos = offset_to_position(text, crab_start).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 7); // "line1: " is 7 chars
        
        // Test after crab (crab is 4 bytes, 2 UTF-16 code units)
        let after_crab = crab_start + 4;
        let pos = offset_to_position(text, after_crab).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 9); // 7 + 2 = 9
        
        // Test start of line 2
        let line2_start = 12;
        let pos = offset_to_position(text, line2_start).unwrap();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);
        
        // Test e + combining accent (e = 1 byte, combining accent = 2 bytes)
        // In UTF-16, this is 2 code units (e + combining accent)
        let e_start = line2_start + 7; // After "line2: "
        let pos = offset_to_position(text, e_start).unwrap();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 7);
        
        // Test after e (before combining accent)
        let after_e = e_start + 1; // e is 1 byte
        let pos = offset_to_position(text, after_e).unwrap();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 8); // 7 + 1 = 8
        
        // Test after combining accent
        let after_combining = after_e + 2; // combining accent is 2 bytes
        let pos = offset_to_position(text, after_combining).unwrap();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 9); // 7 + 1 + 1 = 9
        
        // Test start of line 3
        let line3_start = 23; // 12 + 11
        let pos = offset_to_position(text, line3_start).unwrap();
        assert_eq!(pos.line, 2);
        assert_eq!(pos.character, 0);
        
        // Test family emoji (complex ZWJ sequence)
        let family_start = line3_start + 7;
        let pos = offset_to_position(text, family_start).unwrap();
        assert_eq!(pos.line, 2);
        assert_eq!(pos.character, 7);
        
        // Family emoji is 25 bytes, 11 UTF-16 code units
        // (👨 = 2, ZWJ = 1, 👩 = 2, ZWJ = 1, 👧 = 2, ZWJ = 1, 👦 = 2)
        let after_family = family_start + 25;
        let pos = offset_to_position(text, after_family).unwrap();
        assert_eq!(pos.line, 2);
        assert_eq!(pos.character, 18); // 7 + 11 = 18
    }
    
    // Test that mid-character offsets return None
    #[test]
    fn mid_character_offsets_return_none() {
        let text = "🦀"; // 4 bytes
        
        // Valid offsets: 0 (start) and 4 (end)
        assert!(offset_to_position(text, 0).is_some());
        assert!(offset_to_position(text, 4).is_some());
        
        // Invalid offsets: 1, 2, 3 (mid-character)
        assert!(offset_to_position(text, 1).is_none());
        assert!(offset_to_position(text, 2).is_none());
        assert!(offset_to_position(text, 3).is_none());
    }
}

// --- Property-Based Tests ---
// Feature: world-class-tooling-critical-path
// Property 14: UTF-16 Conversion Correctness
// Property 16: Invalid Span Handling
// Validates: Requirements 5.2, 6.1, 6.2, 6.3

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Strategy for generating valid UTF-8 strings with various character types
    fn text_strategy() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-zA-Z0-9 \n\r🦀你好]*").unwrap()
    }

    // Strategy for generating valid char-boundary offsets within a string
    // This ensures we only generate offsets that are at character boundaries
    fn valid_char_boundary_offsets(text: &str) -> Vec<usize> {
        let mut offsets = vec![0];
        offsets.extend(text.char_indices().map(|(i, _)| i));
        offsets.push(text.len());
        offsets.sort();
        offsets.dedup();
        offsets
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 14: UTF-16 Conversion Correctness
        /// For any valid text and char-boundary offset, converting to position and back should preserve the offset
        #[test]
        fn prop_offset_position_roundtrip(text in text_strategy()) {
            let valid_offsets = valid_char_boundary_offsets(&text);
            for offset in valid_offsets {
                if let Some(pos) = offset_to_position(&text, offset) {
                    if let Some(recovered_offset) = position_to_offset(&text, pos) {
                        // Roundtrip should preserve offset
                        prop_assert_eq!(recovered_offset, offset, 
                            "Roundtrip failed for offset {} in text {:?}", offset, text);
                    }
                }
            }
        }
        
        /// Property 16: Invalid Mid-Character Offsets Return None
        /// For any offset that falls in the middle of a multi-byte character, offset_to_position should return None
        #[test]
        fn prop_invalid_mid_char_offsets_return_none(text in text_strategy()) {
            // Find all invalid offsets (mid-character positions)
            let valid_offsets: std::collections::HashSet<usize> = 
                valid_char_boundary_offsets(&text).into_iter().collect();
            
            for offset in 0..=text.len() {
                if !valid_offsets.contains(&offset) {
                    // This offset is in the middle of a character
                    let result = offset_to_position(&text, offset);
                    prop_assert!(result.is_none(), 
                        "offset_to_position should return None for mid-character offset {} in text {:?}", 
                        offset, text);
                }
            }
        }

        /// Property 14: UTF-16 Conversion with Multi-byte Characters
        /// For any text containing multi-byte characters, UTF-16 positions should be correct
        #[test]
        fn prop_multibyte_utf16_correctness(
            prefix in "[a-zA-Z0-9 ]*",
            emoji in "🦀🎉👍",
            suffix in "[a-zA-Z0-9 ]*"
        ) {
            let text = format!("{}{}{}", prefix, emoji, suffix);
            let emoji_start = prefix.len();
            let emoji_end = emoji_start + emoji.len();
            
            // Position at start of emoji
            if let Some(pos_start) = offset_to_position(&text, emoji_start) {
                // Position at end of emoji
                if let Some(pos_end) = offset_to_position(&text, emoji_end) {
                    // UTF-16 difference should match emoji's UTF-16 length
                    let utf16_diff = pos_end.character - pos_start.character;
                    let expected_utf16_len = emoji.encode_utf16().count() as u32;
                    prop_assert_eq!(utf16_diff, expected_utf16_len);
                }
            }
        }

        /// Property 16: Invalid Span Handling
        /// For any invalid span, span_to_lsp_range should return an error
        #[test]
        fn prop_invalid_span_handling(text in text_strategy()) {
            let text_len = text.len();
            
            // Test out of bounds start
            if text_len > 0 {
                let span = Span::new(text_len + 1, text_len + 2);
                let result = span_to_lsp_range(&span, &text);
                prop_assert!(result.is_err());
            }
            
            // Test out of bounds end
            if text_len > 0 {
                let span = Span::new(0, text_len + 1);
                let result = span_to_lsp_range(&span, &text);
                prop_assert!(result.is_err());
            }
            
            // Test inverted span (start > end)
            if text_len > 1 {
                let span = Span::new(text_len, 0);
                let result = span_to_lsp_range(&span, &text);
                prop_assert!(result.is_err());
            }
        }

        /// Property 14: Span Conversion Preserves Boundaries
        /// For any valid span at char boundaries, converting to LSP range should preserve start/end relationships
        #[test]
        fn prop_span_conversion_preserves_boundaries(text in text_strategy()) {
            let valid_offsets = valid_char_boundary_offsets(&text);
            
            for &start in &valid_offsets {
                for &end in &valid_offsets {
                    if start <= end {
                        let span = Span::new(start, end);
                        if let Ok(range) = span_to_lsp_range(&span, &text) {
                            // Start should be before or equal to end
                            prop_assert!(
                                range.start.line < range.end.line ||
                                (range.start.line == range.end.line && range.start.character <= range.end.character),
                                "Range ordering violated for span {:?} in text {:?}", span, text
                            );
                        }
                    }
                }
            }
        }

        /// Property 14: Empty Spans Convert Correctly
        /// For any valid char-boundary offset, an empty span should convert to a zero-width range
        #[test]
        fn prop_empty_spans(text in text_strategy()) {
            let valid_offsets = valid_char_boundary_offsets(&text);
            for offset in valid_offsets {
                let span = Span::new(offset, offset);
                if let Ok(range) = span_to_lsp_range(&span, &text) {
                    prop_assert_eq!(range.start, range.end,
                        "Empty span at offset {} should produce zero-width range", offset);
                }
            }
        }

        /// Property 14: Line Boundaries Handled Correctly
        /// For any text with newlines, positions should correctly track line numbers
        #[test]
        fn prop_line_boundaries(lines in prop::collection::vec("[a-zA-Z0-9 ]*", 1..10)) {
            let text = lines.join("\n");
            let mut offset = 0;
            
            for (line_num, line) in lines.iter().enumerate() {
                // Position at start of line
                if let Some(pos) = offset_to_position(&text, offset) {
                    prop_assert_eq!(pos.line, line_num as u32);
                    prop_assert_eq!(pos.character, 0);
                }
                
                offset += line.len();
                if line_num < lines.len() - 1 {
                    offset += 1; // newline
                }
            }
        }
    }
}
