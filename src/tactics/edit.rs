use tower_lsp::lsp_types::{WorkspaceEdit, TextEdit, Url, Range};
use std::collections::HashMap;
use crate::document::ByteSpan;

/// Helper to build WorkspaceEdits for tactics.
pub struct EditBuilder {
    uri: Url,
    text: String,
}

impl EditBuilder {
    pub fn new(uri: Url, text: String) -> Self {
        Self { uri, text }
    }

    /// Create a WorkspaceEdit that replaces the given span with new text.
    pub fn replace_span(&self, span: ByteSpan, new_text: String) -> WorkspaceEdit {
        let range = self.span_to_range(span);
        let edit = TextEdit {
            range,
            new_text,
        };

        let mut changes = HashMap::new();
        changes.insert(self.uri.clone(), vec![edit]);

        WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }
    }

    /// Create a WorkspaceEdit that inserts text before the given span.
    pub fn insert_before_span(&self, span: ByteSpan, new_text: String) -> WorkspaceEdit {
        let range = Range {
            start: self.offset_to_position(span.start),
            end: self.offset_to_position(span.start),
        };
        let edit = TextEdit {
            range,
            new_text,
        };

        let mut changes = HashMap::new();
        changes.insert(self.uri.clone(), vec![edit]);

        WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }
    }

    /// Wrap a span with a prefix and suffix.
    pub fn wrap_span(&self, span: ByteSpan, prefix: String, suffix: String) -> WorkspaceEdit {
        let range = self.span_to_range(span);
        let original_text = self.text.get(span.start..span.end).unwrap_or("");
        let new_text = format!("{}{}{}", prefix, original_text, suffix);

        let edit = TextEdit {
            range,
            new_text,
        };

        let mut changes = HashMap::new();
        changes.insert(self.uri.clone(), vec![edit]);

        WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }
    }

    fn span_to_range(&self, span: ByteSpan) -> Range {
        Range {
            start: self.offset_to_position(span.start),
            end: self.offset_to_position(span.end),
        }
    }

    fn offset_to_position(&self, offset: usize) -> tower_lsp::lsp_types::Position {
        crate::span_conversion::offset_to_position(&self.text, offset).unwrap_or_default()
    }
}
