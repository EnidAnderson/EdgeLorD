// Phase 1: Module Snapshot Tests
//
// Tests for Phase 1 semantic caching layer with SniperDB-backed module snapshots.
// These tests verify that module snapshots enable cache reuse across workspace changes.

use std::sync::Arc;
use edgelord_lsp::caching::{ModuleSnapshot, ModuleSnapshotCache};
use comrade_lisp::comrade_workspace::WorkspaceReport;
use codeswitch::fingerprint::HashValue;
use std::time::SystemTime;

// Helper: Create standard fingerprints for testing
fn default_opts_fp() -> HashValue {
    HashValue::hash_with_domain(b"OPTIONS", b"default")
}

fn default_deps_fp() -> HashValue {
    HashValue::hash_with_domain(b"DEPS", b"empty")
}

#[test]
fn test_phase1_module_snapshot_hit_on_all_inputs_match() {
    // INV PHASE-1-MODULE-1: Sound reuse only when all 4 components match
    let db = Arc::new(sniper_db::SniperDatabase::new());
    let mut cache = ModuleSnapshotCache::new(db);

    let file_id = 12345u32;
    let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content1");
    let opts_fp = default_opts_fp();
    let deps_fp = default_deps_fp();

    let fingerprint = [99u8; 32];
    let snapshot = ModuleSnapshot {
        file_id,
        content_hash: content_hash.clone(),
        options_fingerprint: opts_fp.clone(),
        dependency_fingerprint: deps_fp.clone(),
        report: WorkspaceReport {
            diagnostics: Vec::new(),
            fingerprint: Some(fingerprint),
            revision: 1,
            bundle: None,
            proof_state: None,
        },
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    };

    // Insert snapshot
    cache.insert(snapshot.clone());

    // Retrieve with matching 4-component key
    let retrieved = cache.get(file_id, content_hash.clone(), opts_fp.clone(), deps_fp.clone());
    assert!(
        retrieved.is_some(),
        "Module snapshot should be found when all 4 key components match"
    );
    assert_eq!(
        retrieved.unwrap().report.fingerprint,
        Some(fingerprint),
        "Retrieved snapshot should match original"
    );
    assert_eq!(cache.stats().hits, 1, "Should record one cache hit");
}

#[test]
fn test_phase1_module_snapshot_miss_on_content_change() {
    // INV PHASE-1-MODULE-3: Content change invalidates module snapshot
    let db = Arc::new(sniper_db::SniperDatabase::new());
    let mut cache = ModuleSnapshotCache::new(db);

    let file_id = 12345u32;
    let content_hash1 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content1");
    let content_hash2 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content2_different");
    let opts_fp = default_opts_fp();
    let deps_fp = default_deps_fp();

    let snapshot = ModuleSnapshot {
        file_id,
        content_hash: content_hash1.clone(),
        options_fingerprint: opts_fp.clone(),
        dependency_fingerprint: deps_fp.clone(),
        report: WorkspaceReport::default(),
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    };

    cache.insert(snapshot);

    // Query with different content hash
    let retrieved = cache.get(file_id, content_hash2, opts_fp, deps_fp);
    assert!(retrieved.is_none(), "Should miss with different content hash");
    assert_eq!(cache.stats().misses, 1, "Should record one cache miss");
}

#[test]
fn test_phase1_multiple_files_independent_snapshots() {
    // Phase 1: Different files maintain independent snapshots with complete keys
    let db = Arc::new(sniper_db::SniperDatabase::new());
    let mut cache = ModuleSnapshotCache::new(db);

    let file_id1 = 100u32;
    let file_id2 = 200u32;
    let content_hash1 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"file1_content");
    let content_hash2 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"file2_content");
    let opts_fp = default_opts_fp();
    let deps_fp = default_deps_fp();

    let snapshot1 = ModuleSnapshot {
        file_id: file_id1,
        content_hash: content_hash1.clone(),
        options_fingerprint: opts_fp.clone(),
        dependency_fingerprint: deps_fp.clone(),
        report: WorkspaceReport {
            diagnostics: Vec::new(),
            fingerprint: Some([1u8; 32]),
            revision: 1,
            bundle: None,
            proof_state: None,
        },
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    };

    let snapshot2 = ModuleSnapshot {
        file_id: file_id2,
        content_hash: content_hash2.clone(),
        options_fingerprint: opts_fp.clone(),
        dependency_fingerprint: deps_fp.clone(),
        report: WorkspaceReport {
            diagnostics: Vec::new(),
            fingerprint: Some([2u8; 32]),
            revision: 2,
            bundle: None,
            proof_state: None,
        },
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    };

    // Insert both snapshots
    cache.insert(snapshot1);
    cache.insert(snapshot2);

    // Retrieve file1 with matching key
    let retrieved1 = cache.get(file_id1, content_hash1, opts_fp.clone(), deps_fp.clone());
    assert!(retrieved1.is_some());
    assert_eq!(retrieved1.unwrap().report.fingerprint, Some([1u8; 32]));

    // Retrieve file2 with matching key
    let retrieved2 = cache.get(file_id2, content_hash2, opts_fp.clone(), deps_fp.clone());
    assert!(retrieved2.is_some());
    assert_eq!(retrieved2.unwrap().report.fingerprint, Some([2u8; 32]));

    // Both should hit
    assert_eq!(cache.stats().hits, 2);
    assert_eq!(cache.stats().misses, 0);
}

#[test]
fn test_phase1_snapshot_cache_capacity() {
    // Phase 1: Cache respects max_entries limit
    let db = Arc::new(sniper_db::SniperDatabase::new());
    let mut cache = ModuleSnapshotCache::with_max_entries(db, 10);

    let opts_fp = default_opts_fp();

    // Insert more entries than capacity
    for i in 0..20 {
        let file_id = (1000 + i) as u32;
        let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", i.to_string().as_bytes());
        let deps_fp = HashValue::hash_with_domain(b"DEPS", i.to_string().as_bytes());
        let snapshot = ModuleSnapshot {
            file_id,
            content_hash,
            options_fingerprint: opts_fp.clone(),
            dependency_fingerprint: deps_fp,
            report: WorkspaceReport::default(),
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };
        cache.insert(snapshot);
    }

    // Should not exceed max_entries
    assert!(
        cache.len() <= 10,
        "Cache size {} should not exceed max_entries 10",
        cache.len()
    );
}

#[test]
fn test_phase1_snapshot_stats_tracking() {
    // Phase 1: Statistics accurately track hit rate with complete key
    let db = Arc::new(sniper_db::SniperDatabase::new());
    let mut cache = ModuleSnapshotCache::new(db);

    let file_id = 12345u32;
    let content_hash1 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"c1");
    let content_hash2 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"c2");
    let opts_fp = default_opts_fp();
    let deps_fp = default_deps_fp();

    let snapshot = ModuleSnapshot {
        file_id,
        content_hash: content_hash1.clone(),
        options_fingerprint: opts_fp.clone(),
        dependency_fingerprint: deps_fp.clone(),
        report: WorkspaceReport::default(),
        diagnostics: Vec::new(),
        timestamp: SystemTime::now(),
    };

    cache.insert(snapshot);

    // 3 hits with matching key
    for _ in 0..3 {
        cache.get(file_id, content_hash1.clone(), opts_fp.clone(), deps_fp.clone());
    }

    // 2 misses with different content hash
    for _ in 0..2 {
        cache.get(file_id, content_hash2.clone(), opts_fp.clone(), deps_fp.clone());
    }

    let stats = cache.stats();
    assert_eq!(stats.hits, 3);
    assert_eq!(stats.misses, 2);
    assert!(
        (stats.hit_rate() - 0.6).abs() < 0.01,
        "Hit rate should be ~0.6 (3/5)"
    );
}
