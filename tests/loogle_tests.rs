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
        assert!(proposal.id.starts_with("loogle_"));
    }
}
