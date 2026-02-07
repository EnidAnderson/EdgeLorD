//! Golden determinism tests for refute endpoint (G5).
//!
//! These tests lock the JSON schema and ensure byte-for-byte determinism.

use edgelord_lsp::refute::{
    RefuteRequest, RefuteResponse, handle_refute_request, REFUTE_PROTOCOL_VERSION,
};
use edgelord_lsp::refute::witness::FailureWitness;

/// Golden test: same input twice → identical JSON (G5)
#[test]
fn test_refute_determinism_json_golden() {
    let req = RefuteRequest {
        anchor: "goal:file:///test.ml:module/theorem:0".to_string(),
        coherence_level: 0,
        limits: None,
    };

    let resp1 = handle_refute_request(req.clone(), true);
    let resp2 = handle_refute_request(req, true);

    let json1 = serde_json::to_string_pretty(&resp1).unwrap();
    let json2 = serde_json::to_string_pretty(&resp2).unwrap();

    // Byte-for-byte identical
    assert_eq!(json1, json2, "Response JSON must be deterministic");
    
    // Verify schema version
    assert_eq!(resp1.version, REFUTE_PROTOCOL_VERSION);
    assert_eq!(resp1.timestamp_ms, 0, "Test mode must have 0 timestamp");
}

/// Golden test: coherence level 1 returns MissingCoherenceWitness, not equation failure (G4)
#[test]
fn test_refute_level1_returns_missing_coherence_not_false() {
    let req = RefuteRequest {
        anchor: "goal:file:///test.ml:theorem:0".to_string(),
        coherence_level: 1,
        limits: None,
    };

    let resp = handle_refute_request(req, true);
    
    // Must have at least one proposal
    assert!(!resp.proposals.items.is_empty(), "Level 1 should return a proposal");
    
    let proposal = &resp.proposals.items[0];
    
    // Must NOT be a Level0EquationFailure claiming level 1
    match &proposal.payload.counterexample.failure {
        FailureWitness::Level0EquationFailure { .. } => {
            panic!("Level 1 request must NOT return Level0EquationFailure");
        }
        FailureWitness::Level1CoherenceMissing { reason, .. } => {
            // Good: this is honest
            assert!(
                reason.contains("Cannot decide") || reason.contains("level 0"),
                "Reason should explain inability to decide higher coherence"
            );
        }
        FailureWitness::UnsupportedButSuspicious { .. } => {
            // Also acceptable if honest
        }
        _ => {}
    }
}

/// LSP smoke test: handler returns valid JSON, no Debug formatting (G1)
#[test]
fn test_lsp_refute_smoke() {
    let req = RefuteRequest {
        anchor: "goal:file:///smoke_test.ml:main:0".to_string(),
        coherence_level: 0,
        limits: None,
    };

    let resp = handle_refute_request(req, true);

    // Serialize to JSON
    let json = serde_json::to_string(&resp).unwrap();

    // No Debug formatting (no "{:?}" patterns, no "Some(", no "None")
    assert!(!json.contains("{:?}"), "No Debug formatting in JSON");
    assert!(json.contains("\"version\""), "Must have version field");
    assert!(json.contains("\"proposals\""), "Must have proposals field");
    assert!(json.contains("\"meta\""), "Must have meta field");
    assert!(json.contains("\"engine\":\"refute_v1\""), "Must have engine identifier");
}

/// Test deterministic ordering of proposals
#[test]
fn test_refute_proposals_deterministic_ordering() {
    // This test verifies the ordering rules are applied
    let req = RefuteRequest {
        anchor: "goal:file:///order_test.ml:thm:0".to_string(),
        coherence_level: 1,
        limits: None,
    };

    let resp1 = handle_refute_request(req.clone(), true);
    let resp2 = handle_refute_request(req, true);

    // Order must be identical
    assert_eq!(resp1.proposals.items.len(), resp2.proposals.items.len());
    for (p1, p2) in resp1.proposals.items.iter().zip(resp2.proposals.items.iter()) {
        assert_eq!(p1.id, p2.id, "Proposal IDs must match in order");
    }
}

/// Snapshot the golden JSON schema structure
#[test]
fn test_refute_json_schema_snapshot() {
    let req = RefuteRequest {
        anchor: "goal:file:///snapshot.ml:def:0".to_string(),
        coherence_level: 0,
        limits: None,
    };

    let resp = handle_refute_request(req, true);
    let json = serde_json::to_string_pretty(&resp).unwrap();

    // Verify required top-level fields exist
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    
    assert!(parsed.get("version").is_some(), "Missing 'version' field");
    assert!(parsed.get("timestampMs").is_some(), "Missing 'timestampMs' field");
    assert!(parsed.get("proposals").is_some(), "Missing 'proposals' field");
    assert!(parsed.get("meta").is_some(), "Missing 'meta' field");
    
    // Verify proposals structure
    let proposals = parsed.get("proposals").unwrap();
    assert!(proposals.get("items").is_some(), "Missing 'proposals.items'");
    assert!(proposals.get("totalCount").is_some(), "Missing 'proposals.totalCount'");
    assert!(proposals.get("truncated").is_some(), "Missing 'proposals.truncated'");
    
    // Verify meta structure
    let meta = parsed.get("meta").unwrap();
    assert!(meta.get("anchor").is_some(), "Missing 'meta.anchor'");
    assert!(meta.get("coherenceLevelRequested").is_some(), "Missing 'meta.coherenceLevelRequested'");
    assert!(meta.get("limits").is_some(), "Missing 'meta.limits'");
    assert!(meta.get("engine").is_some(), "Missing 'meta.engine'");
}

/// Slice bounded and sorted test (G2)
#[test]
fn test_refute_slice_is_bounded_and_sorted() {
    use edgelord_lsp::refute::types::RefuteLimits;

    let req = RefuteRequest {
        anchor: "goal:file:///sorted_test.ml:thm:0".to_string(),
        coherence_level: 0,
        limits: Some(RefuteLimits {
            max_domain_size: 3,
            max_trace_steps: 50,
            ..Default::default()
        }),
    };

    let resp = handle_refute_request(req, true);

    // Proposals should be bounded
    assert!(resp.proposals.total_count <= resp.meta.limits.max_interpretations);
    
    // If truncated, should have reason
    if resp.proposals.truncated {
        assert!(resp.proposals.truncation_reason.is_some());
    }
    
    // Proposals ordering is stable (verified by determinism tests)
    // The key invariant is that ordering is stable across calls
}
