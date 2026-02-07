use edgelord_lsp::refute::lsp_handler::{handle_refute_request, RefuteRequest, REFUTE_PROTOCOL_VERSION};

#[test]
fn test_refute_pipeline_integration() {
    // 1. Construct request
    let anchor = "goal:file:///test.ml:module/theorem:0".to_string();
    let req = RefuteRequest {
        anchor: anchor.clone(),
        coherence_level: 0,
        limits: None, // Use defaults
    };

    // 2. Call handler (public entry point)
    let resp = handle_refute_request(req, true); // test_mode=true -> timestamp=0

    // 3. Verify protocol fundamentals
    assert_eq!(resp.version, 1, "Protocol version must be 1");
    // timestamp_ms is u64, should be 0 in test mode
    assert_eq!(resp.timestamp_ms, 0, "Timestamp must be 0 in test mode");

    // 4. Verify metadata echo
    assert_eq!(resp.meta.anchor, anchor);
    assert_eq!(resp.meta.coherence_level_requested, 0);
    assert_eq!(resp.meta.engine, "refute_v1");

    // 5. Verify sorting and bounding invariants
    // Even if empty, properties must hold.
    assert!(resp.proposals.total_count >= resp.proposals.items.len());
    
    // If we had items, we would verify sorting order (score desc, etc.)
    // Since current extract_slice is empty, items likely empty, which is valid sorted state.
    if !resp.proposals.items.is_empty() {
        let mut prev_score = f32::INFINITY;
        for item in &resp.proposals.items {
            assert!(item.score <= prev_score, "Proposals must be sorted by score descending");
            prev_score = item.score;

            // DecisionInfo honesty check
            let decision = &item.payload.counterexample.decision;
            if decision.decided {
                assert!(decision.decidable, "If decided, must be decidable");
            }
            if !decision.decidable {
                assert!(!decision.decided, "If not decidable, cannot be decided");
            }
        }
    }
}
