#[cfg(test)]
mod tests {
    use edgelord_lsp::tactics::*; // Import everything from src/tactics/mod.rs
    use crate::refute::types::{BoundedList, StableAnchor, AnchorKind};
    use codeswitch::fingerprint::HashValue;
    use std::collections::BTreeMap;

    // Helper function to create a default ProofState
    fn create_test_proof_state() -> ProofState {
        ProofState {
            goals: BoundedList::from_vec(vec![
                Goal {
                    id: "goal_0".to_string(),
                    anchor: StableAnchor::test(AnchorKind::Goal, "test_file", 0),
                    statement: "(Prop)".to_string(),
                    context: BoundedList::empty(),
                },
                Goal {
                    id: "goal_1".to_string(),
                    anchor: StableAnchor::test(AnchorKind::Goal, "test_file", 1),
                    statement: "(A -> B)".to_string(),
                    context: BoundedList::from_vec(vec![ContextItem::Binding {
                        name: "A".to_string(),
                        type_expr: "Prop".to_string(),
                    }]),
                },
            ]),
            holes: BoundedList::from_vec(vec![Hole {
                id: "hole_0".to_string(),
                anchor: StableAnchor::test(AnchorKind::Hole, "test_file", 0),
                expected_type: Some("Prop".to_string()),
                context: BoundedList::empty(),
            }]),
            definitions_in_scope: BoundedList::empty(),
            rules_in_scope: BoundedList::empty(),
            current_focus_anchor: StableAnchor::test(AnchorKind::Goal, "test_file", 0),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"test_ws"),
        }
    }

    // Helper function to create a TacticInput
    fn create_test_tactic_input(
        target_hole_id: Option<&str>,
        target_goal_id: Option<&str>,
        additional_args: Option<BTreeMap<String, String>>,
    ) -> TacticInput {
        TacticInput {
            current_proof_state: create_test_proof_state(),
            target_hole_id: target_hole_id.map(|s| s.to_string()),
            target_goal_id: target_goal_id.map(|s| s.to_string()),
            additional_args: additional_args.unwrap_or_default(),
            current_document_snapshot_hash: HashValue::hash_with_domain(b"DOC", b"test_doc"),
            current_compile_options_hash: HashValue::hash_with_domain(b"OPTS", b"test_opts"),
        }
    }

    // --- Determinism Tests ---

    #[test]
    fn test_intro_binder_tactic_determinism() {
        let input = create_test_tactic_input(None, Some("0"), Some({
            let mut map = BTreeMap::new();
            map.insert("name".to_string(), "foo".to_string());
            map.insert("type".to_string(), "Type".to_string());
            map
        }));

        let result1 = run_tactic(input.clone(), intro_binder_tactic);
        let result2 = run_tactic(input, intro_binder_tactic);

        assert!(matches!(result1, TacticResult::Success { .. }));
        assert_eq!(result1, result2);

        if let TacticResult::Success { patches: p1, .. } = result1 {
            if let TacticResult::Success { patches: p2, .. } = result2 {
                assert_eq!(p1.len(), 1);
                assert_eq!(p1[0].id, p2[0].id); // Check hash too
            }
        }
    }

    #[test]
    fn test_exact_term_tactic_determinism() {
        let input = create_test_tactic_input(Some("0"), None, Some({
            let mut map = BTreeMap::new();
            map.insert("term".to_string(), "(def x (A -> A))".to_string());
            map
        }));

        let result1 = run_tactic(input.clone(), exact_term_tactic);
        let result2 = run_tactic(input, exact_term_tactic);

        assert!(matches!(result1, TacticResult::Success { .. }));
        assert_eq!(result1, result2);
        if let TacticResult::Success { patches: p1, .. } = result1 {
            if let TacticResult::Success { patches: p2, .. } = result2 {
                assert_eq!(p1.len(), 1);
                assert_eq!(p1[0].id, p2[0].id);
            }
        }
    }

    #[test]
    fn test_rewrite_rule_tactic_determinism() {
        let input = create_test_tactic_input(None, Some("0"), Some({
            let mut map = BTreeMap::new();
            map.insert("rule_id".to_string(), "eq_sym".to_string());
            map.insert("direction".to_string(), "LeftToRight".to_string());
            map.insert("target_span_start".to_string(), "10".to_string());
            map.insert("target_span_end".to_string(), "20".to_string());
            map
        }));

        let result1 = run_tactic(input.clone(), rewrite_rule_tactic);
        let result2 = run_tactic(input, rewrite_rule_tactic);

        assert!(matches!(result1, TacticResult::Success { .. }));
        assert_eq!(result1, result2);
        if let TacticResult::Success { patches: p1, .. } = result1 {
            if let TacticResult::Success { patches: p2, .. } = result2 {
                assert_eq!(p1.len(), 1);
                assert_eq!(p1[0].id, p2[0].id);
            }
        }
    }

    #[test]
    fn test_simp_fuel_tactic_determinism() {
        let input = create_test_tactic_input(None, Some("0"), Some({
            let mut map = BTreeMap::new();
            map.insert("fuel".to_string(), "5".to_string());
            map
        }));

        let result1 = run_tactic(input.clone(), simp_fuel_tactic);
        let result2 = run_tactic(input, simp_fuel_tactic);

        assert!(matches!(result1, TacticResult::Success { .. }));
        assert_eq!(result1, result2);
        if let TacticResult::Success { patches: p1, .. } = result1 {
            if let TacticResult::Success { patches: p2, .. } = result2 {
                assert_eq!(p1.len(), 1);
                assert_eq!(p1[0].id, p2[0].id);
            }
        }
    }

    // --- Trust-Boundary Tests ---

    /// Placeholder for a function that would simulate the kernel's soundness check.
    fn simulate_stonewall_check(patch: &SemanticPatch) -> bool {
        // In a real scenario, this would involve:
        // 1. Deserializing patch.changes into an internal AST representation.
        // 2. Applying these changes to a dummy proof state/document model.
        // 3. Invoking the elaborator/kernel's type checker (e.g., via Q_CHECK_UNIT_V1).
        // 4. Returning true if the new state is sound, false otherwise.

        // For this test, we'll simulate a success based on patch kind.
        // This is a simplification; actual soundness is complex.
        match &patch.kind {
            PatchKind::FillHole { term, .. } => !term.contains("UNSOUND"),
            _ => true, // Assume other patches are sound by default for this mock
        }
    }

    #[test]
    fn test_tactic_produces_sound_patch() {
        let input = create_test_tactic_input(Some("0"), None, Some({
            let mut map = BTreeMap::new();
            map.insert("term".to_string(), "(ok_term A)".to_string());
            map
        }));

        let result = run_tactic(input, exact_term_tactic);

        if let TacticResult::Success { patches, .. } = result {
            assert!(!patches.is_empty());
            assert!(simulate_stonewall_check(&patches[0]), "The generated patch should be considered sound by Stonewall.");
        } else {
            panic!("Tactic failed unexpectedly: {:?}", result);
        }
    }

    #[test]
    fn test_tactic_simulates_unsound_patch_failure() {
        // This test simulates a tactic that internally detects an unsound scenario
        // and returns a TacticFailureReason::SoundnessCheckFailed.
        // In reality, the TacticResult::Success would be passed to EdgeLorD
        // and EdgeLorD would call simulate_stonewall_check and then fail.
        // For this starter slice, the tactic itself can decide to fail.
        let always_fail_unsound_tactic = |_: TacticInput| -> TacticResult {
            TacticResult::Failure {
                reason: TacticFailureReason::SoundnessCheckFailed,
                message: "Simulated: The proposed change would lead to an unsound state.".to_string(),
                diagnostic: None,
            }
        };

        let input = create_test_tactic_input(Some("0"), None, None);
        let result = run_tactic(input, always_fail_unsound_tactic);

        assert!(matches!(result, TacticResult::Failure { reason: TacticFailureReason::SoundnessCheckFailed, .. }));
    }

    // --- "No Text Edits" Tests ---

    #[test]
    fn test_semantic_patch_is_independent_of_text_edits() {
        let input = create_test_tactic_input(Some("0"), None, Some({
            let mut map = BTreeMap::new();
            map.insert("term".to_string(), "(some_maclane_term)".to_string());
            map
        }));

        let result = run_tactic(input, exact_term_tactic);

        if let TacticResult::Success { patches, .. } = result {
            assert!(!patches.is_empty());
            let patch = &patches[0];

            // Assert that the SemanticPatch contains semantic changes
            assert!(!patch.changes.is_empty());
            assert!(matches!(patch.kind, PatchKind::FillHole { .. }));
            if let SemanticChange::UpdateForm { new_form_text, .. } = &patch.changes[0] {
                assert_eq!(new_form_text, "(some_maclane_term)");
            } else {
                panic!("Expected UpdateForm change");
            }

            // Critically, assert that the SemanticPatch itself does NOT contain any
            // LSP-specific TextEdit structures. This is handled by a separate
            // "editor tactic" layer downstream.
            // There's no direct way to check for absence of a type, but we can assert
            // that our SemanticPatch struct definition explicitly lacks TextEdit.
            // This is implicitly checked by the compilation of src/tactics/mod.rs
            // and the absence of TextEdit in its definition.
            // The purpose of this test is more conceptual verification.
            println!("SemanticPatch successfully generated: {:?}", patch);

        } else {
            panic!("Tactic failed unexpectedly: {:?}", result);
        }
    }

    #[test]
    fn test_simp_fuel_exhausted() {
        let input = create_test_tactic_input(None, Some("0"), Some({
            let mut map = BTreeMap::new();
            map.insert("fuel".to_string(), "0".to_string()); // 0 fuel should cause exhaustion
            map
        }));

        let result = run_tactic(input, simp_fuel_tactic);

        assert!(matches!(result, TacticResult::Failure { reason: TacticFailureReason::FuelExhausted, .. }));
    }
}
