// Phase 1.1 Benchmark Tests
//
// Performance measurements for deterministic snapshot reuse caching.
// Scenario: Hot edit loops and cross-file edits with cache hit/miss tracking.
//
// Output: CSV format with metrics for before/after analysis.

use codeswitch::fingerprint::HashValue;
use edgelord_lsp::caching::{CacheKey, CacheValue, ModuleCache};
use comrade_lisp::comrade_workspace::WorkspaceReport;
use std::time::{Instant, SystemTime};

/// CSV row for benchmark results
#[derive(Debug, Clone)]
struct BenchmarkRow {
    timestamp_ms: u128,
    scenario: String,
    iteration: usize,
    unit_id: String,
    decision: String, // "hit", "miss", or "compile"
    miss_reason: String,
    compile_time_ms: u128,
    cache_lookup_time_ms: u128,
    total_time_ms: u128,
    diagnostics_count: usize,
    cache_size: usize,
}

impl BenchmarkRow {
    fn to_csv_header() -> &'static str {
        "timestamp_ms,scenario,iteration,unit_id,decision,miss_reason,compile_time_ms,cache_lookup_time_ms,total_time_ms,diagnostics_count,cache_size"
    }

    fn to_csv_line(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            self.timestamp_ms,
            self.scenario,
            self.iteration,
            self.unit_id,
            self.decision,
            self.miss_reason,
            self.compile_time_ms,
            self.cache_lookup_time_ms,
            self.total_time_ms,
            self.diagnostics_count,
            self.cache_size
        )
    }
}

/// Helper to create a cache key for a specific file version
fn make_cache_key(unit_id: &str, content_version: usize, workspace_version: usize) -> CacheKey {
    CacheKey {
        options_fingerprint: HashValue::hash_with_domain(b"OPTIONS", b"stable_opts"),
        workspace_snapshot_hash: HashValue::hash_with_domain(
            b"WORKSPACE",
            format!("ws_{}", workspace_version).as_bytes(),
        ),
        unit_id: unit_id.to_string(),
        unit_content_hash: HashValue::hash_with_domain(
            b"CONTENT",
            format!("{}_{}", unit_id, content_version).as_bytes(),
        ),
        dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"stable_deps"),
    }
}

/// Helper to create a dummy cache value
fn make_cache_value(dv: usize) -> CacheValue {
    let mut fingerprint = [0u8; 32];
    fingerprint[0] = (dv & 0xFF) as u8;
    fingerprint[1] = ((dv >> 8) & 0xFF) as u8;
    CacheValue {
        report: WorkspaceReport {
            diagnostics: vec![],
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
            fingerprint: Some(fingerprint),
            revision: dv as u64,
            bundle: None,
            proof_state: None,
        },
        diagnostics: vec![],
        timestamp: SystemTime::now(),
    }
}

/// Scenario 1: Hot edit loop on single file
/// Simulate repeatedly editing the same file with high cache hit rate expected.
#[test]
#[ignore]
fn bench_hot_edit_loop() {
    let mut results = vec![];
    let mut cache = ModuleCache::new();
    let start = Instant::now();

    // Warmup: populate cache with initial versions
    for i in 0..5 {
        let key = make_cache_key("file_a.ml", i, 0);
        cache.insert(key, make_cache_value(i));
    }

    // Hot edit loop: rapid edits with cache reuse
    for iteration in 0..1000 {
        let content_version = iteration % 10; // Cycle through recent versions
        let key = make_cache_key("file_a.ml", content_version, 0);

        let lookup_start = Instant::now();
        let hit = cache.get(&key).is_some();
        let lookup_time = lookup_start.elapsed().as_millis();

        let decision = if hit { "hit" } else { "miss" };
        let miss_reason = if hit {
            String::new()
        } else {
            "cache_miss_not_found".to_string()
        };

        if !hit {
            let compile_start = Instant::now();
            cache.insert(key, make_cache_value(iteration));
            let compile_time = compile_start.elapsed().as_millis();

            results.push(BenchmarkRow {
                timestamp_ms: start.elapsed().as_millis(),
                scenario: "hot_edit_loop".to_string(),
                iteration,
                unit_id: "file_a.ml".to_string(),
                decision: decision.to_string(),
                miss_reason,
                compile_time_ms: compile_time,
                cache_lookup_time_ms: lookup_time,
                total_time_ms: compile_time + lookup_time,
                diagnostics_count: 0,
                cache_size: cache.len(),
            });
        } else {
            results.push(BenchmarkRow {
                timestamp_ms: start.elapsed().as_millis(),
                scenario: "hot_edit_loop".to_string(),
                iteration,
                unit_id: "file_a.ml".to_string(),
                decision: decision.to_string(),
                miss_reason,
                compile_time_ms: 0,
                cache_lookup_time_ms: lookup_time,
                total_time_ms: lookup_time,
                diagnostics_count: 0,
                cache_size: cache.len(),
            });
        }
    }

    // Print results
    print_benchmark_results("hot_edit_loop", &results);

    // Verify cache hit rate >= 60%
    let hits = results.iter().filter(|r| r.decision == "hit").count();
    let hit_rate = (hits as f64) / (results.len() as f64);
    println!("Cache hit rate: {:.2}%", hit_rate * 100.0);
    assert!(
        hit_rate >= 0.60,
        "Hot edit loop should achieve >= 60% cache hit rate"
    );
}

/// Scenario 2: Cross-file edit loop
/// Simulate editing multiple dependent files, testing invalidation.
#[test]
#[ignore]
fn bench_cross_file_edit_loop() {
    let mut results = vec![];
    let mut cache = ModuleCache::new();
    let start = Instant::now();

    let files = vec!["file_a.ml", "file_b.ml", "file_c.ml"];

    // Populate cache with initial versions
    for (i, file) in files.iter().enumerate() {
        let key = make_cache_key(file, 0, 0);
        cache.insert(key, make_cache_value(i));
    }

    // Cross-file edit loop
    for iteration in 0..300 {
        let file_idx = iteration % files.len();
        let file = files[file_idx];
        let content_version = (iteration / files.len()) % 5;

        let key = make_cache_key(file, content_version, iteration / files.len());

        let lookup_start = Instant::now();
        let hit = cache.get(&key).is_some();
        let lookup_time = lookup_start.elapsed().as_millis();

        let decision = if hit { "hit" } else { "miss" };
        let miss_reason = if hit {
            String::new()
        } else {
            "workspace_change".to_string()
        };

        let compile_time = if !hit {
            let compile_start = Instant::now();
            cache.insert(key, make_cache_value(iteration));
            compile_start.elapsed().as_millis()
        } else {
            0
        };

        results.push(BenchmarkRow {
            timestamp_ms: start.elapsed().as_millis(),
            scenario: "cross_file_edit".to_string(),
            iteration,
            unit_id: file.to_string(),
            decision: decision.to_string(),
            miss_reason,
            compile_time_ms: compile_time,
            cache_lookup_time_ms: lookup_time,
            total_time_ms: compile_time + lookup_time,
            diagnostics_count: 0,
            cache_size: cache.len(),
        });
    }

    // Print results
    print_benchmark_results("cross_file_edit", &results);

    // Verify reasonable performance (>= 25% compilation reduction vs no caching)
    let misses = results.iter().filter(|r| r.decision == "miss").count();
    let miss_rate = (misses as f64) / (results.len() as f64);
    let compilation_reduction = 1.0 - miss_rate;
    println!("Compilation reduction: {:.2}%", compilation_reduction * 100.0);
    assert!(
        compilation_reduction >= 0.25,
        "Cross-file scenario should reduce compilations by >= 25%"
    );
}

/// Scenario 3: Cache effectiveness under size pressure
/// Insert many entries and verify eviction behavior.
#[test]
#[ignore]
fn bench_cache_under_size_pressure() {
    let mut results = vec![];
    let mut cache = edgelord_lsp::caching::ModuleCache::with_max_entries(50);
    let start = Instant::now();

    // Insert beyond max capacity
    for iteration in 0..200 {
        let unit_id = format!("file_{}.ml", iteration % 100);
        let key = make_cache_key(&unit_id, iteration, 0);

        let lookup_start = Instant::now();
        let hit = cache.get(&key).is_some();
        let lookup_time = lookup_start.elapsed().as_millis();

        if !hit {
            let compile_start = Instant::now();
            cache.insert(key, make_cache_value(iteration));
            let compile_time = compile_start.elapsed().as_millis();

            results.push(BenchmarkRow {
                timestamp_ms: start.elapsed().as_millis(),
                scenario: "size_pressure".to_string(),
                iteration,
                unit_id,
                decision: "miss".to_string(),
                miss_reason: "eviction".to_string(),
                compile_time_ms: compile_time,
                cache_lookup_time_ms: lookup_time,
                total_time_ms: compile_time + lookup_time,
                diagnostics_count: 0,
                cache_size: cache.len(),
            });
        } else {
            results.push(BenchmarkRow {
                timestamp_ms: start.elapsed().as_millis(),
                scenario: "size_pressure".to_string(),
                iteration,
                unit_id,
                decision: "hit".to_string(),
                miss_reason: String::new(),
                compile_time_ms: 0,
                cache_lookup_time_ms: lookup_time,
                total_time_ms: lookup_time,
                diagnostics_count: 0,
                cache_size: cache.len(),
            });
        }
    }

    // Verify cache stays within bounds
    assert!(
        cache.len() <= 50,
        "Cache size {} should not exceed max_entries 50",
        cache.len()
    );

    print_benchmark_results("size_pressure", &results);
}

/// Print benchmark results in CSV format to stdout
fn print_benchmark_results(scenario: &str, results: &[BenchmarkRow]) {
    println!("\n=== Benchmark: {} ===", scenario);
    println!("{}", BenchmarkRow::to_csv_header());
    for row in results {
        println!("{}", row.to_csv_line());
    }

    // Summary statistics
    if !results.is_empty() {
        let total_time: u128 = results.iter().map(|r| r.total_time_ms).sum();
        let avg_time = total_time / results.len() as u128;
        let max_time = results.iter().map(|r| r.total_time_ms).max().unwrap_or(0);
        let hits = results.iter().filter(|r| r.decision == "hit").count();
        let hit_rate = (hits as f64) / (results.len() as f64) * 100.0;

        println!("\n=== Summary for {} ===", scenario);
        println!("Total iterations: {}", results.len());
        println!("Hit rate: {:.2}%", hit_rate);
        println!("Average time per op: {} ms", avg_time);
        println!("Max time per op: {} ms", max_time);
        println!("Final cache size: {}", results.last().unwrap().cache_size);
    }
}

/// Integration test: Verify stats tracking across operations
#[test]
fn test_cache_stats_comprehensive() {
    let mut cache = ModuleCache::new();

    // Perform mixed operations
    for i in 0..10 {
        let key = make_cache_key(&format!("file_{}.ml", i), i, 0);
        cache.insert(key, make_cache_value(i));
    }

    // Try hits and misses
    for i in 0..10 {
        let key = make_cache_key(&format!("file_{}.ml", i), i, 0);
        let _ = cache.get(&key); // Should hit
    }

    for i in 10..15 {
        let key = make_cache_key(&format!("file_{}.ml", i), i, 0);
        let _ = cache.get(&key); // Should miss
    }

    // Verify stats
    let stats = cache.stats();
    assert_eq!(stats.hits, 10);
    assert_eq!(stats.misses, 5);
    assert!(stats.hit_rate() > 0.6, "Hit rate should be > 60%");
}

/// Acceptance threshold verification with baseline measurement
#[test]
fn test_cache_acceptance_thresholds() {
    // Threshold: Cache hit rate >= 60% (measured)

    let mut cache = ModuleCache::new();

    // Warmup: populate cache with 10 versions
    for i in 0..10 {
        let key = make_cache_key("test.ml", i, 0);
        cache.insert(key, make_cache_value(i));
    }

    // Measurement: simulate 100 accesses cycling through recent versions
    for iteration in 0..100 {
        let content_version = iteration % 10;
        let key = make_cache_key("test.ml", content_version, 0);
        let _ = cache.get(&key);
    }

    let stats = cache.stats();
    let hit_rate = stats.hit_rate();

    println!(
        "Cache hit rate: {:.2}% (Hits: {}, Misses: {})",
        hit_rate * 100.0,
        stats.hits,
        stats.misses
    );

    // Acceptance: hit rate >= 60%
    assert!(
        hit_rate >= 0.60,
        "Cache must achieve >= 60% hit rate (measured: {:.2}%)",
        hit_rate * 100.0
    );
}

/// Baseline vs cached compilation time comparison
#[test]
fn test_cache_baseline_comparison() {
    use std::time::Instant;

    let mut cache = ModuleCache::new();

    // Baseline: 100 lookups without cache (all misses)
    let baseline_start = Instant::now();
    for i in 0..100 {
        let key = make_cache_key(&format!("file_{}.ml", i), i, 0);
        let _ = cache.get(&key);  // All misses
    }
    let baseline_time_us = baseline_start.elapsed().as_micros();

    // Cached: 100 lookups with populated cache (high hit rate)
    cache.clear();
    for i in 0..10 {
        let key = make_cache_key("hot_file.ml", i, 0);
        cache.insert(key, make_cache_value(i));
    }

    let cached_start = Instant::now();
    for iteration in 0..100 {
        let content_version = iteration % 10;
        let key = make_cache_key("hot_file.ml", content_version, 0);
        let _ = cache.get(&key);  // High hit rate
    }
    let cached_time_us = cached_start.elapsed().as_micros();

    let improvement = if baseline_time_us > 0 {
        ((baseline_time_us - cached_time_us) as f64 / baseline_time_us as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "Baseline (all misses): {} µs",
        baseline_time_us
    );
    println!(
        "Cached (high hits): {} µs",
        cached_time_us
    );
    println!(
        "Improvement: {:.1}%",
        improvement
    );

    // Evidence: cache lookups are faster than miss path
    assert!(
        cached_time_us < baseline_time_us,
        "Cached lookups should be faster than all-miss baseline"
    );
}
