/// Tests for Loogle indexing and search functionality
#[cfg(test)]
mod tests {
    use edgelord_lsp::loogle::{LoogleIndex, WorkspaceIndexer, check_applicability};

    #[test]
    fn test_loogle_index_and_search() {
        let index = LoogleIndex::new_in_memory().unwrap();
        
        // Index a test lemma
        index.index_lemma(
            "nat_add_comm",
            "add_nat_nat",
            "add_comm_result",
            "Commutativity of addition",
            Some("stdlib"),
        ).unwrap();
        
        // Search for it
        let results = index.search("add_nat_nat", 10).unwrap();
        assert!(results.len() > 0, "Should find at least one result");
        assert_eq!(results[0].name, "nat_add_comm");
    }

    #[test]
    fn test_applicability_check() {
        use edgelord_lsp::loogle::LoogleResult;
        
        let lemma = LoogleResult {
            name: "nat_add_comm".to_string(),
            rationale: "Structural match: add(?a, ?b)".to_string(),
            doc: "Commutativity of addition".to_string(),
        };
        
        let result = check_applicability(&lemma, "add(x, y)");
        
        // Should not be applicable with our current simple matching
        // (this will improve when we add proper unification)
        assert!(result.pedagogical_rationale.contains("nat_add_comm"));
    }

    #[test]
    fn test_proposal_generation() {
        use edgelord_lsp::loogle::{LoogleResult, ApplicabilityResult, to_proposal};
        
        let lemma = LoogleResult {
            name: "test_lemma".to_string(),
            rationale: "Test".to_string(),
            doc: "A test lemma".to_string(),
        };
        
        let applicability = ApplicabilityResult {
            applicable: true,
            confidence: 0.9,
            unification_preview: Some("?x := Nat".to_string()),
            pedagogical_rationale: "Applies because...".to_string(),
        };
        
        let proposal = to_proposal(lemma, applicability, "test_anchor".to_string());
        
        assert_eq!(proposal.payload.name, "test_lemma");
        assert_eq!(proposal.score, 0.9);
        // ID should be deterministic hash, not UUID
        assert!(proposal.id.starts_with("loogle_"));
        assert!(!proposal.id.contains("-")); // UUIDs have dashes
        assert_eq!(proposal.id.len(), "loogle_".len() + 32); // 16 bytes hex-encoded
    }
    
    #[test]
    fn test_proposal_id_determinism() {
        use edgelord_lsp::loogle::{LoogleResult, ApplicabilityResult, to_proposal};
        
        let lemma = LoogleResult {
            name: "deterministic_test".to_string(),
            rationale: "Test".to_string(),
            doc: "Testing determinism".to_string(),
        };
        
        let applicability = ApplicabilityResult {
            applicable: true,
            confidence: 0.75,
            unification_preview: None,
            pedagogical_rationale: "Test rationale".to_string(),
        };
        
        // Generate two proposals with identical inputs
        let proposal1 = to_proposal(lemma.clone(), applicability.clone(), "anchor_123".to_string());
        let proposal2 = to_proposal(lemma, applicability, "anchor_123".to_string());
        
        // IDs should be identical (content-addressed)
        assert_eq!(proposal1.id, proposal2.id);
        
        // Verify it's a hex string of the right format
        assert!(proposal1.id.starts_with("loogle_"));
        let hash_part = &proposal1.id["loogle_".len()..];
        assert_eq!(hash_part.len(), 32); // 16 bytes as hex
        assert!(hash_part.chars().all(|c| c.is_ascii_hexdigit()));
    }
    
    #[test]
    fn test_fingerprint_determinism() {
        use edgelord_lsp::loogle::{compute_fingerprint, LOOGLE_FP_VERSION};
        use tcb_core::ast::MorphismTerm;
        use tcb_core::id_minting::GeneratorId;
        
        // Create a test term using Default
        let term = MorphismTerm::Generator {
            id: GeneratorId::default(),
            inputs: vec![],
            outputs: vec![],
        };
        
        // Compute fingerprint twice - must be byte-for-byte identical
        let fp1 = compute_fingerprint(&term);
        let fp2 = compute_fingerprint(&term);
        
        assert_eq!(fp1, fp2, "Fingerprint must be deterministic");
        
        // Should have version prefix
        assert!(fp1.starts_with(&format!("v{}:", LOOGLE_FP_VERSION)));
    }
    
    #[test]
    fn test_fingerprint_version_tag() {
        use edgelord_lsp::loogle::{compute_fingerprint, LOOGLE_FP_VERSION};
        use tcb_core::ast::MorphismTerm;
        
        // Create a Hole term - HoleId is just u32
        let term = MorphismTerm::Hole(7);
        let fp = compute_fingerprint(&term);
        
        // Verify version tag format v{N}:{fingerprint}
        assert!(fp.starts_with("v"), "Fingerprint must start with v");
        let parts: Vec<&str> = fp.splitn(2, ':').collect();
        assert_eq!(parts.len(), 2, "Fingerprint must have version:payload format");
        
        let version_str = &parts[0][1..]; // Strip 'v'
        let version: u32 = version_str.parse().expect("Version must be numeric");
        assert_eq!(version, LOOGLE_FP_VERSION);
    }
    
    #[test]
    fn test_fingerprint_no_debug_format() {
        use edgelord_lsp::loogle::compute_fingerprint;
        use tcb_core::ast::MorphismTerm;
        use tcb_core::doctrine::DoctrineKey;
        use tcb_core::id_minting::GeneratorId;
        
        // Create a term with InDoctrine wrapper - this previously used {:?}
        let inner = MorphismTerm::Generator {
            id: GeneratorId::default(),
            inputs: vec![],
            outputs: vec![],
        };
        let term = MorphismTerm::InDoctrine {
            doctrine: DoctrineKey::placeholder(), // Key 0
            term: Box::new(inner),
        };
        
        let fp = compute_fingerprint(&term);
        
        // Should NOT contain Debug-style output like "DoctrineKey(0)"
        assert!(!fp.contains("DoctrineKey"), 
            "Fingerprint must not contain Debug output: {}", fp);
        
        // Should contain stable numeric format like "doc:0:"
        assert!(fp.contains("doc:0:"), 
            "Fingerprint should use stable as_u32() format: {}", fp);
    }
}
