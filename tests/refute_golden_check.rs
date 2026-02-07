use edgelord_lsp::refute::lsp_handler::{RefuteResponse, RefuteMeta, REFUTE_PROTOCOL_VERSION};
use edgelord_lsp::refute::types::{BoundedList, RefuteLimits, DecisionInfo, JumpTarget, ByteSpan, StableAnchor, AnchorKind, TruncationReason};
use edgelord_lsp::refute::witness::{Counterexample, CounterexamplePayload, FailureWitness, InterpretationSummary, InterpretationKind, DiagramBoundary};
use edgelord_lsp::refute::probe::ProbeKey;
use edgelord_lsp::refute::slice::SliceSummary;
use edgelord_lsp::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary};

#[test]
fn test_manual_golden_json_strict() {
    // 1. Construct a fully populated response
    let probe = ProbeKey {
        id: "test_probe".to_string(),
        level: 0,
        description: "Test Probe".to_string(),
    };

    let witness = FailureWitness::Level1CoherenceMissing {
        boundary: DiagramBoundary {
            source_path: "src".to_string(),
            target_path: "tgt".to_string(),
            required_cell: "alpha".to_string(),
            anchor: Some("anchor:1".to_string()),
        },
        diagram: Some("A -> B".to_string()),
        expected_2cell: "alpha".to_string(),
        reason: "missing".to_string(),
    };

    let counterexample = Counterexample {
        probe: probe.clone(),
        interpretation: InterpretationSummary {
            kind: InterpretationKind::FiniteCategory { objects: 2, morphism_count: 3 },
            description: "Interp 1".to_string(),
            domain_size: Some(2),
        },
        failure: witness,
        level: 1,
        slice_summary: SliceSummary {
            obligations_included: 1,
            rules_included: 2,
            defs_included: 0,
            trace_steps_used: 10,
            truncated: true,
            reason: Some(TruncationReason::Budget),
        },
        decision: DecisionInfo {
            decidable: false,
            decided: false,
            reason: Some("undecidable".to_string()),
        },
    };

    let proposal = Proposal {
        id: "prop:1".to_string(),
        anchor: "anchor:1".to_string(),
        kind: ProposalKind::Refutation,
        payload: CounterexamplePayload::new(counterexample),
        evidence: EvidenceSummary {
            rationale: "rationale".to_string(),
            trace_nodes: vec![],
        },
        status: ProposalStatus::Advisory,
        reconstruction: None,
        score: 0.5,
        truncated: false,
    };

    let response = RefuteResponse {
        version: 1,
        timestamp_ms: 12345,
        proposals: BoundedList::from_vec(vec![proposal]),
        meta: RefuteMeta {
            anchor: "anchor:1".to_string(),
            coherence_level_requested: 1,
            limits: RefuteLimits::default(),
            engine: "test_engine".to_string(),
        },
    };

    // 2. Serialize
    let json = serde_json::to_string_pretty(&response).unwrap();

    // 3. Verify key fields exist in camelCase
    assert!(json.contains("\"timestampMs\": 12345"), "Missing timestampMs");
    assert!(json.contains("\"totalCount\": 1"), "Missing totalCount"); 
    assert!(json.contains("\"coherenceLevelRequested\": 1"), "Missing coherenceLevelRequested");
    assert!(json.contains("\"domainSize\": 2"), "Missing domainSize");
    assert!(json.contains("\"morphismCount\": 3"), "Missing morphismCount"); // Inside InterpretationKind
    assert!(json.contains("\"sourcePath\": \"src\""), "Missing sourcePath"); // Inside DiagramBoundary
    assert!(json.contains("\"traceStepsUsed\": 10"), "Missing traceStepsUsed"); // Inside SliceSummary
    assert!(json.contains("\"reason\": \"budget\""), "Missing reason: budget (SliceSummary)"); // Inside SliceSummary it's 'reason'
    assert!(json.contains("\"truncationReason\": null"), "Missing truncationReason: null (BoundedList)"); // Inside BoundedList it's 'truncationReason'
    // TruncationReason::Budget -> "budget" because of rename_all="camelCase"?
    // Actually enum variants usually default to name unless renamed.
    // TruncationReason has #[serde(rename_all = "camelCase")] so "Budget" -> "budget"
    assert!(json.contains("\"budget\""), "Enum variant should be camelCase 'budget' but found something else? JSON:\n{}", json);
}
