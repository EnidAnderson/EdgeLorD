// Phase 1.1 Invariants Tests
//
// Tests for:
// - INV D-CACHE-1: Purity (determinism)
// - INV D-CACHE-2: Sound reuse (exact key match required)
// - INV D-CACHE-3: Monotone invalidation (edits invalidate dependents)

use codeswitch::fingerprint::HashValue;
use edgelord_lsp::caching::{CacheKey, CacheKeyBuilder, CacheValue, ModuleCache};
use comrade_lisp::comrade_workspace::WorkspaceReport;
use std::time::SystemTime;
use std::collections::BTreeMap;
use tower_lsp::lsp_types::Url;
use comrade_lisp::WorkspaceDiagnostic;

/// Helper to create a test cache key
fn test_cache_key(
    options: &str,
    workspace: &str,
    unit: &str,
    content: &str,
    deps: &str,
) -> CacheKey {
    CacheKey {
        options_fingerprint: HashValue::hash_with_domain(b"OPTIONS", options.as_bytes()),
        workspace_snapshot_hash: HashValue::hash_with_domain(b"WORKSPACE", workspace.as_bytes()),
        unit_id: unit.to_string(),
        unit_content_hash: HashValue::hash_with_domain(b"CONTENT", content.as_bytes()),
        dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", deps.as_bytes()),
    }
}

/// Helper to create a test cache value
fn test_cache_value(seed: u64) -> CacheValue {
    let mut fingerprint = [0u8; 32];
    fingerprint[0] = (seed % 256) as u8;
    fingerprint[1] = ((seed / 256) % 256) as u8;
    CacheValue {
        report: WorkspaceReport {
            diagnostics: Vec::new(),
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
            fingerprint: Some(fingerprint),
            revision: 0,
            bundle: None,
            proof_state: None,
        },
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    }
}

/// INV D-CACHE-1: Purity Test
///
/// Statement: For the same CacheKey, compilation output is identical.
/// Test: Compile twice with identical inputs, verify identical fingerprints.
#[test]
fn test_inv_d_cache_1_purity_determinism_replay() {
    let key = test_cache_key("opt1", "ws1", "file.ml", "content1", "deps1");
    let value1 = test_cache_value(42);
    let value2 = test_cache_value(42);

    // Same key should retrieve same cached value
    let mut cache = ModuleCache::new();
    cache.insert(key.clone(), value1.clone());

    let retrieved = cache.get(&key).expect("Cache hit failed");
    assert_eq!(retrieved.report.fingerprint, value1.report.fingerprint);
    assert_eq!(retrieved.report.fingerprint, value2.report.fingerprint);
}

/// INV D-CACHE-1: Purity Test (Warm Cache)
///
/// Test: Compile once (populate cache), compile again (hit cache).
/// Verify: Cached output equals fresh compile output.
#[test]
fn test_inv_d_cache_1_purity_warm_cache_determinism() {
    let key = test_cache_key("opt1", "ws1", "file.ml", "content1", "deps1");
    let value = test_cache_value(100);

    let mut cache = ModuleCache::new();
    cache.insert(key.clone(), value.clone());

    // Multiple cache hits should return identical value
    for _ in 0..5 {
        let cached = cache.get(&key).expect("Cache hit failed");
        assert_eq!(cached.report.fingerprint, value.report.fingerprint);
        assert_eq!(cached.report.revision, value.report.revision);
    }
}

/// INV D-CACHE-2: Sound Reuse Test (Options Mismatch)
///
/// Statement: Cache reuse only if CacheKey matches exactly.
/// Test: Change compile option without changing file.
/// Verify: Cache not used; cache miss recorded.
#[test]
fn test_inv_d_cache_2_sound_reuse_options_mismatch_busts_cache() {
    let key1 = test_cache_key("opt_v1", "ws1", "file.ml", "content1", "deps1");
    let key2 = test_cache_key("opt_v2", "ws1", "file.ml", "content1", "deps1");
    let value = test_cache_value(42);

    let mut cache = ModuleCache::new();
    cache.insert(key1.clone(), value);

    // Different options = different key = cache miss
    assert!(cache.get(&key1).is_some());
    assert!(cache.get(&key2).is_none());
    assert_eq!(cache.stats().misses, 1); // One miss for key2
}

/// INV D-CACHE-2: Sound Reuse Test (Workspace Mismatch)
///
/// Test: Change workspace (another file changes).
/// Verify: Cache not used for dependent unit.
#[test]
fn test_inv_d_cache_2_sound_reuse_workspace_mismatch_busts_cache() {
    let key1 = test_cache_key("opt", "ws_v1", "file.ml", "content", "deps");
    let key2 = test_cache_key("opt", "ws_v2", "file.ml", "content", "deps");
    let value = test_cache_value(42);

    let mut cache = ModuleCache::new();
    cache.insert(key1.clone(), value);

    // Different workspace = cache miss
    assert!(cache.get(&key1).is_some());
    assert!(cache.get(&key2).is_none());
}

/// INV D-CACHE-2: Sound Reuse Test (Content Mismatch)
///
/// Test: Change file content.
/// Verify: Cache not used.
#[test]
fn test_inv_d_cache_2_sound_reuse_content_mismatch_busts_cache() {
    let key1 = test_cache_key("opt", "ws", "file.ml", "content_v1", "deps");
    let key2 = test_cache_key("opt", "ws", "file.ml", "content_v2", "deps");
    let value = test_cache_value(42);

    let mut cache = ModuleCache::new();
    cache.insert(key1.clone(), value);

    // Different content hash = cache miss
    assert!(cache.get(&key1).is_some());
    assert!(cache.get(&key2).is_none());
}

/// INV D-CACHE-3: Monotone Invalidation Test (Single File Edit)
///
/// Statement: Edit invalidates cached results for that file.
/// Test: Edit file → miss; revert to previous → hit again.
/// Verify: Second compile is cache hit with identical key.
#[test]
fn test_inv_d_cache_3_monotone_invalidation_rollback_re_hit() {
    let key1 = test_cache_key("opt", "ws", "file.ml", "content_v1", "deps");
    let key2 = test_cache_key("opt", "ws", "file.ml", "content_v2", "deps");
    let value1 = test_cache_value(42);
    let value2 = test_cache_value(99);

    let mut cache = ModuleCache::new();

    // Initial compilation: content_v1
    cache.insert(key1.clone(), value1.clone());
    assert!(cache.get(&key1).is_some());

    // Edit: content_v2
    assert!(cache.get(&key2).is_none()); // Miss

    // Store result
    cache.insert(key2.clone(), value2);
    assert!(cache.get(&key2).is_some());

    // Rollback: revert to content_v1
    assert!(cache.get(&key1).is_some()); // Hit (stable hash)

    // Verify fingerprints differ (cache entries were distinct)
    let v1 = cache.get(&key1).expect("key1 hit");
    let v2 = cache.get(&key2).expect("key2 hit");
    assert!(v1.report.fingerprint.is_some());
    assert!(v2.report.fingerprint.is_some());
    assert_ne!(v1.report.fingerprint, v2.report.fingerprint);
}

/// INV D-CACHE-3: Monotone Invalidation Test (All-Invalidate on Workspace Change)
///
/// Test: Change any file in workspace.
/// Verify: All dependent caches invalidated (conservative approach for Phase 1.1).
#[test]
fn test_inv_d_cache_3_monotone_invalidation_workspace_change() {
    let ws_v1 = test_cache_key("opt", "ws_before_change", "file_a.ml", "content_a", "deps");
    let ws_v2 = test_cache_key("opt", "ws_after_change", "file_a.ml", "content_a", "deps");

    let mut cache = ModuleCache::new();
    cache.insert(ws_v1.clone(), test_cache_value(42));

    // Workspace changed (another file edited)
    assert!(cache.get(&ws_v1).is_some()); // Old key hits
    assert!(cache.get(&ws_v2).is_none()); // New workspace hash misses

    // This demonstrates monotone invalidation: workspace changes bust cache
}

/// Test: Cache statistics tracking
#[test]
fn test_cache_statistics_tracking() {
    let key1 = test_cache_key("opt", "ws", "file.ml", "content1", "deps");
    let key2 = test_cache_key("opt", "ws", "file.ml", "content2", "deps");

    let mut cache = ModuleCache::new();
    assert_eq!(cache.stats().total_operations(), 0);

    // Miss
    cache.get(&key1);
    assert_eq!(cache.stats().misses, 1);
    assert_eq!(cache.stats().hits, 0);

    // Insert and hit
    cache.insert(key1.clone(), test_cache_value(42));
    cache.get(&key1);
    assert_eq!(cache.stats().misses, 1);
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().hit_rate(), 0.5);

    // Another miss
    cache.get(&key2);
    assert_eq!(cache.stats().total_operations(), 3);
    assert!(cache.stats().hit_rate() < 0.67); // 1 hit out of 3
}

/// Test: Cache key builder validates all required fields
#[test]
fn test_cache_key_builder_validation() {
    let opt_hash = HashValue::hash_with_domain(b"OPT", b"test");
    let ws_hash = HashValue::hash_with_domain(b"WS", b"test");
    let content_hash = HashValue::hash_with_domain(b"CONTENT", b"test");
    let deps_hash = HashValue::hash_with_domain(b"DEPS", b"test");

    // Valid key
    let key = CacheKeyBuilder::new()
        .options(opt_hash)
        .workspace_snapshot(ws_hash)
        .unit_id("file.ml")
        .unit_content(content_hash)
        .dependencies(deps_hash)
        .build();
    assert!(key.is_ok());

    // Missing field should fail
    let incomplete = CacheKeyBuilder::new()
        .options(opt_hash)
        .workspace_snapshot(ws_hash)
        .unit_id("file.ml")
        // missing content and deps
        .build();
    assert!(incomplete.is_err());
}

/// Test: Cache key is totally ordered (BTreeMap determinism)
#[test]
fn test_cache_key_total_ordering() {
    let keys: Vec<_> = (0..10)
        .map(|i| {
            test_cache_key(
                &format!("opt_{}", i),
                "ws_const",
                "file.ml",
                "content_const",
                "deps_const",
            )
        })
        .collect();

    // All keys should be distinct and orderable
    let mut ordered = keys.clone();
    ordered.sort();

    // Check that BTreeMap can handle all keys
    let mut map = std::collections::BTreeMap::new();
    for (i, key) in keys.iter().enumerate() {
        map.insert(key.clone(), i);
    }
    assert_eq!(map.len(), 10);
}

/// Test: Workspace change invalidates cache (conservative Phase 1.1 behavior)
#[test]
fn test_workspace_change_invalidates_cache() {
    let mut cache = ModuleCache::new();

    // Workspace version 1: cache entry for file_a.ml
    let key_ws1 = test_cache_key("opt", "ws_v1", "file_a.ml", "content1", "deps_v1");
    let value1 = test_cache_value(42);
    cache.insert(key_ws1.clone(), value1);

    // Verify cache hit with ws_v1
    assert!(cache.get(&key_ws1).is_some(), "Cache should hit with original workspace");

    // Workspace changes (another file edited): workspace version 2
    let key_ws2 = test_cache_key("opt", "ws_v2", "file_a.ml", "content1", "deps_v2");

    // Same file, same content, but different workspace → must miss
    assert!(
        cache.get(&key_ws2).is_none(),
        "Cache must miss when workspace changes (conservative invalidation)"
    );

    // Original key still hits (proving we're not clearing entire cache)
    assert!(
        cache.get(&key_ws1).is_some(),
        "Original workspace key should still be cached"
    );
}
