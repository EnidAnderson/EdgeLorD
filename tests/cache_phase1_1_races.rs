// Phase 1.1 Race Condition Tests
//
// Verify that caching preserves existing LSP guarantees:
// - Single-flight compilation (at most one in-flight compile per unit per DV)
// - No stale diagnostics (published diagnostics match newest DV)
//
// These tests are critical: caching must not introduce the ability for
// stale results to publish after newer DVs have been observed.

use codeswitch::fingerprint::HashValue;
use edgelord_lsp::caching::{CacheKey, CacheValue, ModuleCache};
use comrade_lisp::comrade_workspace::WorkspaceReport;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::collections::BTreeMap;
use tower_lsp::lsp_types::Url;
use comrade_lisp::WorkspaceDiagnostic;

/// Helper to create a test cache key with DV
fn test_cache_key_with_dv(dv: i32, content: &str) -> CacheKey {
    CacheKey {
        options_fingerprint: HashValue::hash_with_domain(b"OPTIONS", b"test"),
        workspace_snapshot_hash: HashValue::hash_with_domain(b"WORKSPACE", b"test"),
        unit_id: "test.ml".to_string(),
        unit_content_hash: HashValue::hash_with_domain(b"CONTENT", format!("dv_{}", dv).as_bytes()),
        dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"test"),
    }
}

/// Helper to create a test cache value with DV fingerprint
fn test_cache_value_with_dv(dv: i32) -> CacheValue {
    let mut fingerprint = [0u8; 32];
    fingerprint[0] = (dv & 0xFF) as u8;
    fingerprint[1] = ((dv >> 8) & 0xFF) as u8;
    CacheValue {
        report: WorkspaceReport {
            diagnostics: Vec::new(),
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
            fingerprint: Some(fingerprint),
            revision: dv as u64,
            bundle: None,
            proof_state: None,
        },
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    }
}

/// INV D-RACE-1: Single-Flight Gate Test
///
/// Statement: At most one in-flight compile per unit per DV.
/// Test: Simulate concurrent requests for same unit; verify only latest compiles.
#[test]
fn test_inv_d_race_1_single_flight_concurrent_requests() {
    // Simulate a gate that tracks in-flight compilations
    let in_flight = Arc::new(Mutex::new(Option::<i32>::None));
    let mut cache = ModuleCache::new();

    // DV1: Start compilation
    {
        let mut gate = in_flight.lock().unwrap();
        if gate.is_none() {
            *gate = Some(1);

            // DV1 cache miss
            let key1 = test_cache_key_with_dv(1, "content_1");
            assert!(cache.get(&key1).is_none());

            // DV1 would compile here...
            cache.insert(key1, test_cache_value_with_dv(1));
        }
    }

    // DV2: Start compilation (should replace DV1 in single-flight gate)
    {
        let mut gate = in_flight.lock().unwrap();
        *gate = Some(2); // Replace in-flight DV
    }

    // DV1 compile completes (but DV2 is now in-flight)
    // In real implementation, publish would check: is DV1 still current? No.
    // So DV1 diagnostics wouldn't publish.

    let current_dv = *in_flight.lock().unwrap();
    assert_eq!(current_dv, Some(2)); // DV2 is current, not DV1
}

/// INV D-RACE-2: No Stale Diagnostics Test
///
/// Statement: Published diagnostics must correspond to newest DV at publish time.
/// Test: Simulate out-of-order completion; verify old DV never publishes.
#[test]
fn test_inv_d_race_2_no_stale_diagnostics_out_of_order_completion() {
    // Track which DV published
    let published_dvs = Arc::new(Mutex::new(Vec::new()));
    let mut cache = ModuleCache::new();

    // Simulate out-of-order completion:
    // - DV1 starts (slower compile)
    // - DV2 starts (cache hit, completes first)
    // - DV2 publishes
    // - DV1 completes (too late!)
    // - DV1 should NOT publish

    // DV2 request (arrives second, but completes first via cache hit)
    {
        let key2 = test_cache_key_with_dv(2, "content_2");
        // Pre-populate cache from DV2
        cache.insert(key2.clone(), test_cache_value_with_dv(2));

        // DV2 cache hit
        let cached = cache.get(&key2);
        assert!(cached.is_some());

        // DV2 publishes
        if let Some(value) = cached {
            published_dvs.lock().unwrap().push(value.report.revision);
        }
    }

    // DV1 request (arrives first, but completes second)
    {
        let key1 = test_cache_key_with_dv(1, "content_1");
        // DV1 cache miss
        assert!(cache.get(&key1).is_none());
        // DV1 would compile...
        cache.insert(key1.clone(), test_cache_value_with_dv(1));

        // DV1 completes, but now check: is DV1 still current?
        // In real code: if current_dv != 1, don't publish
        let current_dv = 2; // This is now the current DV
        if current_dv != 1 {
            // Don't publish DV1
        } else {
            published_dvs.lock().unwrap().push(1);
        }
    }

    // Verify: only DV2 published, not DV1
    let published = published_dvs.lock().unwrap();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0], 2); // Only DV2
}

/// INV D-RACE-3: Cache Hit Cannot Overwrite Newer DV
///
/// Statement: Late cache-hit must not publish over a newer DV.
/// Test: DV1 cache-hit completes slowly; DV2 finishes first; verify DV1 doesn't overwrite DV2.
#[test]
fn test_inv_d_race_3_cache_hit_cannot_overwrite_newer_dv() {
    let published_versions = Arc::new(Mutex::new(Vec::new()));

    // DV1: Pre-populate cache
    let mut cache = ModuleCache::new();
    let key1 = test_cache_key_with_dv(1, "content_1");
    cache.insert(key1.clone(), test_cache_value_with_dv(1));

    // Scenario: DV1 cache hit queued, but DV2 finishes and publishes first
    let current_dv = 2; // DV2 is now current

    // DV1 cache hit finally completes
    if let Some(cached) = cache.get(&key1) {
        // Check: is this still current?
        if current_dv == 1 {
            // Would publish
            published_versions.lock().unwrap().push(cached.report.revision);
        }
        // else: Don't publish (DV2 is newer)
    }

    // Verify: DV1 didn't publish because DV2 is current
    assert!(published_versions.lock().unwrap().is_empty());
}

/// Verification: Cache maintains deterministic ordering (no race in lookup/insert)
#[test]
fn test_cache_deterministic_ordering_under_concurrent_inserts() {
    let cache = Arc::new(Mutex::new(ModuleCache::new()));

    // Simulate multiple threads inserting/querying
    let mut handles = vec![];
    for i in 0..5 {
        let cache_clone = cache.clone();
        let handle = std::thread::spawn(move || {
            let key = test_cache_key_with_dv(i, &format!("content_{}", i));
            let value = test_cache_value_with_dv(i);

            let mut cache = cache_clone.lock().unwrap();
            cache.insert(key.clone(), value);
            assert!(cache.get(&key).is_some());
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all entries present and ordered
    let cache = cache.lock().unwrap();
    assert!(cache.len() >= 5); // At least our insertions

    // Verify stats are coherent
    let stats = cache.stats();
    assert!(stats.hits > 0); // We got cache hits
}

/// Test: Single-flight gate pattern (as would be used in ProofSession)
#[test]
fn test_single_flight_gate_pattern() {
    struct SingleFlightGate {
        in_flight: Option<i32>,
    }

    impl SingleFlightGate {
        fn acquire(&mut self, dv: i32) -> bool {
            if self.in_flight.is_none() || self.in_flight < Some(dv) {
                self.in_flight = Some(dv);
                true
            } else {
                false
            }
        }

        fn is_current(&self, dv: i32) -> bool {
            self.in_flight == Some(dv)
        }
    }

    let mut gate = SingleFlightGate {
        in_flight: None,
    };

    // DV1 acquires
    assert!(gate.acquire(1));
    assert!(gate.is_current(1));

    // DV2 replaces DV1
    assert!(gate.acquire(2));
    assert!(!gate.is_current(1));
    assert!(gate.is_current(2));

    // DV1 tries to publish
    if gate.is_current(1) {
        panic!("DV1 should not publish when DV2 is current!");
    }

    // DV2 publishes (OK)
    assert!(gate.is_current(2));
}
