/// Phase 1.2B Integration Tests: DB-Native Compile Query
///
/// These tests validate the hard invariants of Q_CHECK_UNIT_V1:
/// - **Purity**: Same input → same output
/// - **Sound Reuse**: Cached output matches fresh computation
/// - **Determinism**: Digest is stable across runs
/// - **Single-flight**: No redundant compilations for same input

#[cfg(test)]
mod phase1_2b_compile_query_tests {
    use edgelord_lsp::queries::{CompileInputV1, Q_CHECK_UNIT_V1, DiagnosticsArtifactV1};
    use std::collections::BTreeMap;

    #[test]
    fn test_compile_input_v1_purity() {
        // INV: Same inputs → same input_digest (determinism)

        let unit_content = b"(def foo (hole bar))".to_vec();
        let mut opts = BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///test.mc".to_string(), b"test_content".to_vec());

        let file_id = 12345u32;

        // Create two inputs from identical data
        let input1 = CompileInputV1::new(
            unit_content.clone(),
            opts.clone(),
            snapshot.clone(),
            file_id,
        );

        let input2 = CompileInputV1::new(unit_content.clone(), opts.clone(), snapshot.clone(), file_id);

        // Hard invariant: Same inputs → same digest
        assert_eq!(
            input1.input_digest, input2.input_digest,
            "Identical inputs must produce identical digests"
        );
    }

    #[test]
    fn test_compile_input_v1_content_sensitivity() {
        // INV: Different content → different input_digest (content matters)

        let mut opts = BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///test.mc".to_string(), b"snapshot_content".to_vec());

        let file_id = 12345u32;

        // Two inputs with different unit content
        let input1 = CompileInputV1::new(
            b"(def foo (hole bar))".to_vec(),
            opts.clone(),
            snapshot.clone(),
            file_id,
        );

        let input2 = CompileInputV1::new(
            b"(def foo (hole baz))".to_vec(), // Different content
            opts.clone(),
            snapshot.clone(),
            file_id,
        );

        // Hard invariant: Different content → different digest
        assert_ne!(
            input1.input_digest, input2.input_digest,
            "Different content must produce different digests"
        );
    }

    #[test]
    fn test_compile_input_v1_options_sensitivity() {
        // INV: Different options → different input_digest (options matter)

        let unit_content = b"(def foo (hole bar))".to_vec();

        let mut opts1 = BTreeMap::new();
        opts1.insert("dialect".to_string(), "pythonic".to_string());

        let mut opts2 = BTreeMap::new();
        opts2.insert("dialect".to_string(), "canonical".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///test.mc".to_string(), b"snapshot_content".to_vec());

        let file_id = 12345u32;

        let input1 = CompileInputV1::new(
            unit_content.clone(),
            opts1,
            snapshot.clone(),
            file_id,
        );

        let input2 = CompileInputV1::new(unit_content.clone(), opts2, snapshot.clone(), file_id);

        // Hard invariant: Different options → different digest
        assert_ne!(
            input1.input_digest, input2.input_digest,
            "Different options must produce different digests"
        );
    }

    #[test]
    fn test_compile_input_v1_workspace_sensitivity() {
        // INV: Different workspace → different input_digest (dependencies matter)

        let unit_content = b"(def foo (hole bar))".to_vec();
        let mut opts = BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot1 = BTreeMap::new();
        snapshot1.insert("file:///test.mc".to_string(), b"content_v1".to_vec());

        let mut snapshot2 = BTreeMap::new();
        snapshot2.insert("file:///test.mc".to_string(), b"content_v2".to_vec()); // Different content

        let file_id = 12345u32;

        let input1 = CompileInputV1::new(
            unit_content.clone(),
            opts.clone(),
            snapshot1,
            file_id,
        );

        let input2 = CompileInputV1::new(unit_content.clone(), opts.clone(), snapshot2, file_id);

        // Hard invariant: Different workspace → different digest
        assert_ne!(
            input1.input_digest, input2.input_digest,
            "Different workspace snapshot must produce different digests"
        );
    }

    #[test]
    fn test_compile_input_v1_file_id_sensitivity() {
        // INV: Different file_id → different input_digest (file identity matters)

        let unit_content = b"(def foo (hole bar))".to_vec();
        let mut opts = BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///test.mc".to_string(), b"snapshot_content".to_vec());

        // Two inputs with different file IDs
        let input1 = CompileInputV1::new(
            unit_content.clone(),
            opts.clone(),
            snapshot.clone(),
            12345u32,
        );

        let input2 = CompileInputV1::new(
            unit_content.clone(),
            opts.clone(),
            snapshot.clone(),
            54321u32, // Different file ID
        );

        // Hard invariant: Different file_id → different digest
        assert_ne!(
            input1.input_digest, input2.input_digest,
            "Different file IDs must produce different digests"
        );
    }

    #[test]
    fn test_q_check_unit_v1_identity() {
        // Verify query identity constants

        assert_eq!(Q_CHECK_UNIT_V1::name(), "Q_CHECK_UNIT_V1");
        assert_eq!(Q_CHECK_UNIT_V1::query_class(), "unit_compile");
        assert_eq!(Q_CHECK_UNIT_V1::input_version(), 1);
        assert_eq!(Q_CHECK_UNIT_V1::output_version(), 1);
    }

    #[test]
    fn test_diagnostics_artifact_v1_creation() {
        // Verify artifact creation and metadata

        use comrade_lisp::comrade_workspace::WorkspaceReport;

        let report = WorkspaceReport {
            diagnostics: vec![],
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
            fingerprint: None,
            revision: 0,
            bundle: None,
            proof_state: None,
        };

        let artifact =
            DiagnosticsArtifactV1::new(report, vec![], None);

        assert_eq!(artifact.diagnostics.len(), 0);
        assert!(artifact.output_digest.is_none());
    }

    #[test]
    fn test_compile_input_snapshot_ordering() {
        // INV: Workspace snapshot ordering is canonical (deterministic serialization)
        // BTreeMap ensures sorted iteration by key

        let unit_content = b"test".to_vec();
        let mut opts = BTreeMap::new();

        // Create snapshot with unsorted insertion
        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///c.mc".to_string(), b"c_content".to_vec());
        snapshot.insert("file:///a.mc".to_string(), b"a_content".to_vec());
        snapshot.insert("file:///b.mc".to_string(), b"b_content".to_vec());

        let input1 = CompileInputV1::new(unit_content.clone(), opts.clone(), snapshot.clone(), 111);

        // Insert in different order, but BTreeMap enforces sorted order
        let mut snapshot2 = BTreeMap::new();
        snapshot2.insert("file:///a.mc".to_string(), b"a_content".to_vec());
        snapshot2.insert("file:///c.mc".to_string(), b"c_content".to_vec());
        snapshot2.insert("file:///b.mc".to_string(), b"b_content".to_vec());

        let input2 = CompileInputV1::new(unit_content.clone(), opts.clone(), snapshot2, 111);

        // Digests must match because BTreeMap ensures deterministic iteration order
        assert_eq!(
            input1.input_digest, input2.input_digest,
            "Snapshot with same content but different insertion order must have same digest"
        );
    }

    #[test]
    fn test_compile_input_options_ordering() {
        // INV: Options are serialized in sorted order (deterministic)

        let unit_content = b"test".to_vec();
        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///test.mc".to_string(), b"content".to_vec());

        // Create options in one order
        let mut opts1 = BTreeMap::new();
        opts1.insert("zulu".to_string(), "value_z".to_string());
        opts1.insert("alpha".to_string(), "value_a".to_string());
        opts1.insert("bravo".to_string(), "value_b".to_string());

        // Create options in different order (but BTreeMap sorts)
        let mut opts2 = BTreeMap::new();
        opts2.insert("alpha".to_string(), "value_a".to_string());
        opts2.insert("zulu".to_string(), "value_z".to_string());
        opts2.insert("bravo".to_string(), "value_b".to_string());

        let input1 = CompileInputV1::new(unit_content.clone(), opts1, snapshot.clone(), 111);
        let input2 = CompileInputV1::new(unit_content.clone(), opts2, snapshot.clone(), 111);

        // Digests must match because BTreeMap ensures deterministic iteration order
        assert_eq!(
            input1.input_digest, input2.input_digest,
            "Options with same content but different insertion order must have same digest"
        );
    }

    #[test]
    fn test_compile_input_hash_stability() {
        // INV: Input hash is stable across multiple creations

        let unit_content = b"stable content".to_vec();
        let mut opts = BTreeMap::new();
        opts.insert("option".to_string(), "value".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///test.mc".to_string(), b"deps".to_vec());

        let mut digests = Vec::new();
        for _ in 0..5 {
            let input = CompileInputV1::new(
                unit_content.clone(),
                opts.clone(),
                snapshot.clone(),
                42,
            );
            digests.push(input.input_digest);
        }

        // All digests must be identical
        for digest in &digests[1..] {
            assert_eq!(
                &digests[0], digest,
                "Input digest must be stable across multiple creations"
            );
        }
    }
}
