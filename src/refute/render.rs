//! Witness rendering (MVVM separation).
//!
//! Keeps witnesses structural; renders to pretty text at the boundary.

use crate::refute::witness::{Counterexample, FailureWitness};

/// Render a counterexample to human-readable text.
pub fn render_counterexample(cx: &Counterexample) -> String {
    let mut out = String::new();
    
    out.push_str(&format!("## Counterexample ({})\n\n", cx.probe.description));
    out.push_str(&format!("**Interpretation**: {}\n", cx.interpretation.description));
    if let Some(size) = cx.interpretation.domain_size {
        out.push_str(&format!("**Domain size**: {}\n", size));
    }
    out.push_str(&format!("**Coherence level**: {}\n\n", cx.level));
    
    out.push_str("### Failure\n\n");
    out.push_str(&render_failure_witness(&cx.failure));
    
    out.push_str("\n### Slice Summary\n\n");
    out.push_str(&format!("- Obligations: {}\n", cx.slice_summary.obligations_included));
    out.push_str(&format!("- Rules: {}\n", cx.slice_summary.rules_included));
    out.push_str(&format!("- Definitions: {}\n", cx.slice_summary.defs_included));
    if cx.slice_summary.truncated {
        out.push_str(&format!("- *Truncated*: {:?}\n", cx.slice_summary.reason));
    }
    
    out
}

/// Render a failure witness to human-readable text.
pub fn render_failure_witness(witness: &FailureWitness) -> String {
    match witness {
        FailureWitness::Level0EquationFailure {
            lhs,
            rhs,
            explanation,
            jump_targets: _,
        } => {
            let mut out = format!("**Equation failure**:\n```\n{} ≠ {}\n```\n\n", lhs, rhs);
            if !explanation.items.is_empty() {
                out.push_str("**Explanation**:\n");
                for (i, step) in explanation.items.iter().enumerate() {
                    out.push_str(&format!("{}. {}\n", i + 1, step));
                }
            }
            out
        }
        FailureWitness::Level1CoherenceMissing {
            boundary,
            diagram,
            expected_2cell,
            reason,
        } => {
            let diagram_str = diagram.as_deref().unwrap_or(&boundary.source_path);
            format!(
                "**Coherence missing**:\n\nDiagram:\n```\n{}\n```\n\nExpected 2-cell: `{}`\n\nReason: {}\n",
                diagram_str, expected_2cell, reason
            )
        }
        FailureWitness::Level2HigherCoherenceFailure {
            expected_higher_cell,
            reason,
        } => {
            format!(
                "**Higher coherence failure**:\n\nExpected: `{}`\n\nReason: {}\n",
                expected_higher_cell, reason
            )
        }
        FailureWitness::UnsupportedButSuspicious { reason } => {
            format!("**Not refuted** (within bounds):\n\n{}\n", reason)
        }
    }
}

// ============================================================================
// Explain Integration (G7)
// ============================================================================

/// A simplified ExplanationNode for refutation witnesses.
///
/// This mirrors the structure in `explain::view` but is standalone
/// for the refute module to avoid circular dependencies.
#[derive(Debug, Clone)]
pub struct RefuteExplanationNode {
    pub label: String,
    pub kind: String,
    /// Jump target as byte span (converted to UTF-16 at LSP boundary).
    pub jump_target: Option<(usize, usize)>,
    pub children: Vec<RefuteExplanationNode>,
}

impl RefuteExplanationNode {
    pub fn leaf(label: String, kind: &str) -> Self {
        Self {
            label,
            kind: kind.to_string(),
            jump_target: None,
            children: vec![],
        }
    }
    
    pub fn with_jump(mut self, start: usize, end: usize) -> Self {
        self.jump_target = Some((start, end));
        self
    }
}

/// Convert a failure witness to an explanation tree.
///
/// **Invariant**: All labels PrettyCtx-rendered, jump targets byte-offset.
/// UTF-16 conversion happens at LSP boundary via `byte_to_utf16_offset`.
pub fn witness_to_explanation_tree(witness: &FailureWitness) -> RefuteExplanationNode {
    match witness {
        FailureWitness::Level0EquationFailure {
            lhs,
            rhs,
            explanation,
            jump_targets,
        } => {
            let mut children = vec![];
            
            // LHS node
            let lhs_node = RefuteExplanationNode::leaf(
                format!("LHS: {}", lhs),
                "term",
            );
            children.push(lhs_node);
            
            // RHS node
            let rhs_node = RefuteExplanationNode::leaf(
                format!("RHS: {}", rhs),
                "term",
            );
            children.push(rhs_node);
            
            // Explanation steps
            for (i, step) in explanation.items.iter().enumerate() {
                let step_node = RefuteExplanationNode::leaf(
                    format!("Step {}: {}", i + 1, step),
                    "explanation",
                );
                children.push(step_node);
            }
            
            // Add jump targets as children
            for (i, target) in jump_targets.items.iter().enumerate() {
                if let Some(span) = &target.span {
                    let label = target.label.as_deref().unwrap_or("Location");
                    let target_node = RefuteExplanationNode::leaf(
                        format!("{} {}", label, i + 1),
                        target.kind.as_deref().unwrap_or("jump"),
                    ).with_jump(span.start, span.end);
                    children.push(target_node);
                }
            }
            
            RefuteExplanationNode {
                label: "Equation Failure".to_string(),
                kind: "failure".to_string(),
                jump_target: None,
                children,
            }
        }
        FailureWitness::Level1CoherenceMissing {
            boundary,
            diagram,
            expected_2cell,
            reason,
        } => {
            let children = vec![
                RefuteExplanationNode::leaf(
                    format!("Diagram: {}", diagram.as_deref().unwrap_or(&boundary.source_path)), 
                    "diagram"
                ),
                RefuteExplanationNode::leaf(format!("Expected: {}", expected_2cell), "obligation"),
                RefuteExplanationNode::leaf(format!("Reason: {}", reason), "explanation"),
            ];
            
            RefuteExplanationNode {
                label: "Missing Coherence".to_string(),
                kind: "coherence".to_string(),
                jump_target: None,
                children,
            }
        }
        FailureWitness::Level2HigherCoherenceFailure {
            expected_higher_cell,
            reason,
        } => {
            let children = vec![
                RefuteExplanationNode::leaf(format!("Expected: {}", expected_higher_cell), "obligation"),
                RefuteExplanationNode::leaf(format!("Reason: {}", reason), "explanation"),
            ];
            
            RefuteExplanationNode {
                label: "Higher Coherence Failure".to_string(),
                kind: "coherence".to_string(),
                jump_target: None,
                children,
            }
        }
        FailureWitness::UnsupportedButSuspicious { reason } => {
            RefuteExplanationNode::leaf(format!("Not refuted: {}", reason), "info")
        }
    }
}

/// Validate that all jump targets in a tree are within bounds.
///
/// Returns `true` if all spans are valid.
pub fn validate_tree_spans(node: &RefuteExplanationNode, text_len: usize) -> bool {
    #[allow(clippy::collapsible_if)]
    if let Some((start, end)) = node.jump_target {
        if start > end || end > text_len {
            return false;
        }
    }
    node.children.iter().all(|c| validate_tree_spans(c, text_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refute::probe::ProbeKey;
    use crate::refute::slice::SliceSummary;
    use crate::refute::types::{BoundedList, JumpTarget};

    #[test]
    fn test_render_equation_failure() {
        let witness = FailureWitness::Level0EquationFailure {
            lhs: "f ∘ g".to_string(),
            rhs: "g ∘ f".to_string(),
            explanation: BoundedList::from_vec(vec![
                "f and g do not commute in category with 2 objects".to_string(),
            ]),
            jump_targets: BoundedList::empty(),
        };
        
        let rendered = render_failure_witness(&witness);
        assert!(rendered.contains("f ∘ g"));
        assert!(rendered.contains("g ∘ f"));
        assert!(rendered.contains("do not commute"));
    }

    #[test]
    fn test_witness_to_explanation_tree() {
        let witness = FailureWitness::Level0EquationFailure {
            lhs: "a".to_string(),
            rhs: "b".to_string(),
            explanation: BoundedList::from_vec(vec!["not equal".to_string()]),
            jump_targets: BoundedList::from_vec(vec![
                JumpTarget::from_span(10, 20).with_label("test"),
            ]),
        };
        
        let tree = witness_to_explanation_tree(&witness);
        assert_eq!(tree.label, "Equation Failure");
        assert_eq!(tree.children.len(), 4); // LHS, RHS, step, jump target
        
        // Check jump target is included
        let jump_child = &tree.children[3];
        assert_eq!(jump_child.jump_target, Some((10, 20)));
    }

    #[test]
    fn test_utf16_jump_target_validity() {
        // Test that validate_tree_spans correctly identifies valid/invalid spans
        let valid_tree = RefuteExplanationNode {
            label: "test".to_string(),
            kind: "test".to_string(),
            jump_target: Some((0, 50)),
            children: vec![
                RefuteExplanationNode::leaf("child".to_string(), "c").with_jump(10, 30),
            ],
        };
        
        // Valid with text_len = 100
        assert!(validate_tree_spans(&valid_tree, 100));
        
        // Invalid with text_len = 40 (child end > 40)
        assert!(!validate_tree_spans(&valid_tree, 40));
        
        // Invalid span (start > end)
        let invalid_tree = RefuteExplanationNode {
            label: "bad".to_string(),
            kind: "test".to_string(),
            jump_target: Some((50, 10)), // Invalid: start > end
            children: vec![],
        };
        assert!(!validate_tree_spans(&invalid_tree, 100));
    }
}
