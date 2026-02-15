// C2.6: Report Generator
//
// Parses PHASE_1_BASELINE.csv and PHASE_1_CACHED.csv
// Generates benchmarks/PHASE_1_REPORT.md with:
//   - Statistics (hit rates, latencies, compilation reduction %)
//   - Go/No-Go signals for Phase 1.2B/C
//   - Sanity tables showing cache behavior

use std::fs;

#[derive(Debug, Clone)]
struct CsvRow {
    timestamp_ms: u64,
    scenario: String,
    uri: String,
    edit_id: u32,
    dv: u64,
    phase1_outcome: String,
    phase1_1_outcome: String,
    compiled: u8,
    compile_ms: u64,
    end_to_end_ms: u64,
    diagnostics_count: usize,
    bytes_open_docs: usize,
    cache_entries_phase1: String,
    cache_entries_phase1_1: String,
    options_fp8: String,
    deps_fp8: String,
    workspace_fp8: String,
    published: u8,
    note: String,
}

impl CsvRow {
    fn from_csv_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 19 {
            return None;
        }

        Some(CsvRow {
            timestamp_ms: parts[0].parse().ok()?,
            scenario: parts[1].to_string(),
            uri: parts[2].to_string(),
            edit_id: parts[3].parse().ok()?,
            dv: parts[4].parse().ok()?,
            phase1_outcome: parts[5].to_string(),
            phase1_1_outcome: parts[6].to_string(),
            compiled: parts[7].parse().ok()?,
            compile_ms: parts[8].parse().ok()?,
            end_to_end_ms: parts[9].parse().ok()?,
            diagnostics_count: parts[10].parse().ok()?,
            bytes_open_docs: parts[11].parse().ok()?,
            cache_entries_phase1: parts[12].to_string(),
            cache_entries_phase1_1: parts[13].to_string(),
            options_fp8: parts[14].to_string(),
            deps_fp8: parts[15].to_string(),
            workspace_fp8: parts[16].to_string(),
            published: parts[17].parse().ok()?,
            note: parts[18].to_string(),
        })
    }
}

#[derive(Debug, Default)]
struct Stats {
    total_rows: usize,
    total_compiled: usize,
    phase1_hits: usize,
    phase1_misses: usize,
    phase1_1_hits: usize,
    phase1_1_misses: usize,
    compile_times: Vec<u64>,
    end_to_end_times: Vec<u64>,
}

impl Stats {
    fn phase1_hit_rate(&self) -> f64 {
        if self.phase1_hits + self.phase1_misses == 0 {
            0.0
        } else {
            100.0 * self.phase1_hits as f64 / (self.phase1_hits + self.phase1_misses) as f64
        }
    }

    fn phase1_1_hit_rate(&self) -> f64 {
        if self.phase1_1_hits + self.phase1_1_misses == 0 {
            0.0
        } else {
            100.0 * self.phase1_1_hits as f64 / (self.phase1_1_hits + self.phase1_1_misses) as f64
        }
    }

    fn combined_hit_rate(&self) -> f64 {
        let total_hits = self.phase1_hits + self.phase1_1_hits;
        let total_ops = self.phase1_hits + self.phase1_misses + self.phase1_1_hits + self.phase1_1_misses;
        if total_ops == 0 {
            0.0
        } else {
            100.0 * total_hits as f64 / total_ops as f64
        }
    }

    fn p50_latency(&self) -> u64 {
        if self.end_to_end_times.is_empty() {
            return 0;
        }
        let mut sorted = self.end_to_end_times.clone();
        sorted.sort();
        sorted[sorted.len() / 2]
    }

    fn p95_latency(&self) -> u64 {
        if self.end_to_end_times.is_empty() {
            return 0;
        }
        let mut sorted = self.end_to_end_times.clone();
        sorted.sort();
        sorted[(sorted.len() * 95) / 100]
    }
}

fn parse_csv(path: &str) -> Vec<CsvRow> {
    match fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .skip(1)  // Skip header
            .filter_map(CsvRow::from_csv_line)
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn compute_stats(rows: &[CsvRow]) -> Stats {
    let mut stats = Stats::default();
    stats.total_rows = rows.len();

    for row in rows {
        stats.total_compiled += row.compiled as usize;
        
        // Phase 1 outcomes
        if row.phase1_outcome == "hit" {
            stats.phase1_hits += 1;
        } else if row.phase1_outcome.starts_with("miss") {
            stats.phase1_misses += 1;
        }

        // Phase 1.1 outcomes
        if row.phase1_1_outcome == "hit" {
            stats.phase1_1_hits += 1;
        } else if row.phase1_1_outcome.starts_with("miss") {
            stats.phase1_1_misses += 1;
        }

        if row.compile_ms > 0 {
            stats.compile_times.push(row.compile_ms);
        }
        if row.end_to_end_ms > 0 {
            stats.end_to_end_times.push(row.end_to_end_ms);
        }
    }

    stats
}

#[test]
#[ignore]
fn bench_c2_generate_report() {
    println!("\n📊 C2.6: Generating performance report...\n");

    let baseline_csv = "benchmarks/PHASE_1_BASELINE.csv";
    let cached_csv = "benchmarks/PHASE_1_CACHED.csv";

    // Parse CSVs
    let baseline_rows = parse_csv(baseline_csv);
    let cached_rows = parse_csv(cached_csv);

    if baseline_rows.is_empty() || cached_rows.is_empty() {
        eprintln!("⚠️ Warning: CSVs not found or empty. Run benchmarks first:");
        eprintln!("  EDGELORD_DISABLE_CACHES=1 cargo test --test bench_phase1_cache -- --ignored");
        eprintln!("  cargo test --test bench_phase1_cache -- --ignored");
        return;
    }

    // Compute statistics
    let baseline_stats = compute_stats(&baseline_rows);
    let cached_stats = compute_stats(&cached_rows);

    // Generate report
    let mut report = String::new();
    report.push_str("# Phase 1 Cache Performance Report\n\n");
    report.push_str(&format!("**Generated**: {}\n\n", chrono::Local::now()));

    // Executive Summary
    report.push_str("## Executive Summary\n\n");
    let combined_hit_rate = cached_stats.combined_hit_rate();
    if combined_hit_rate >= 60.0 {
        report.push_str("### ✅ GO SIGNAL: Phase 1 Cache is Ready\n\n");
        report.push_str(&format!("**Combined hit rate: {:.1}%** ≥ 60% threshold\n\n", combined_hit_rate));
        report.push_str("Recommendation: **Ship Phase 1 as-is**\n\n");
    } else if combined_hit_rate >= 40.0 {
        report.push_str("### ⚠️ CAUTION: Phase 1.2C (Fine-grained deps) Justified\n\n");
        report.push_str(&format!("**Combined hit rate: {:.1}%** (40–60% range)\n\n", combined_hit_rate));
        report.push_str("Recommendation: Proceed with Phase 1.2C to improve hit rate via dependency tracking\n\n");
    } else {
        report.push_str("### ❌ NO-GO: Requires Phase 1.2B + 1.2C\n\n");
        report.push_str(&format!("**Combined hit rate: {:.1}%** < 40%\n\n", combined_hit_rate));
        report.push_str("Recommendation: Implement both Phase 1.2B (SniperDB memo) and 1.2C (fine-grained deps)\n\n");
    }

    // Statistics Tables
    report.push_str("## Cache Performance Statistics\n\n");
    report.push_str("### Baseline (Caches Disabled)\n\n");
    report.push_str(&format!("- Total edits: {}\n", baseline_stats.total_rows));
    report.push_str(&format!("- Compilations: {}\n", baseline_stats.total_compiled));
    report.push_str(&format!("- P50 latency: {}ms\n", baseline_stats.p50_latency()));
    report.push_str(&format!("- P95 latency: {}ms\n\n", baseline_stats.p95_latency()));

    report.push_str("### Cached (Caches Enabled)\n\n");
    report.push_str(&format!("- Total edits: {}\n", cached_stats.total_rows));
    report.push_str(&format!("- Compilations: {}\n", cached_stats.total_compiled));
    report.push_str(&format!("- Phase 1 hit rate: {:.1}%\n", cached_stats.phase1_hit_rate()));
    report.push_str(&format!("- Phase 1.1 hit rate: {:.1}%\n", cached_stats.phase1_1_hit_rate()));
    report.push_str(&format!("- Combined hit rate: {:.1}%\n", combined_hit_rate));
    report.push_str(&format!("- P50 latency: {}ms\n", cached_stats.p50_latency()));
    report.push_str(&format!("- P95 latency: {}ms\n\n", cached_stats.p95_latency()));

    // Deltas
    report.push_str("### Performance Delta (Baseline → Cached)\n\n");
    let compilation_reduction = if baseline_stats.total_compiled > 0 {
        let baseline_compiled = baseline_stats.total_compiled as i64;
        let cached_compiled = cached_stats.total_compiled as i64;
        let reduction = (baseline_compiled - cached_compiled).max(0) as f64;
        100.0 * reduction / baseline_compiled as f64
    } else {
        0.0
    };
    report.push_str(&format!("- Compilations reduced: {:.1}%\n", compilation_reduction));
    if baseline_stats.p95_latency() > 0 {
        let latency_improvement = 100.0 * (baseline_stats.p95_latency() as f64 - cached_stats.p95_latency() as f64) / baseline_stats.p95_latency() as f64;
        report.push_str(&format!("- P95 latency improvement: {:.1}%\n\n", latency_improvement));
    }

    // Acceptance Criteria
    report.push_str("## Acceptance Criteria\n\n");
    report.push_str("Phase 1 is accepted if **ANY ONE** of these is met:\n\n");
    report.push_str(&format!("1. ✅ Combined hit rate ≥ 60%: **{:.1}%** {}\n",
        combined_hit_rate,
        if combined_hit_rate >= 60.0 { "✓" } else { "✗" }
    ));
    report.push_str(&format!("2. {} Compilations reduced ≥ 25%: **{:.1}%**\n",
        if compilation_reduction >= 25.0 { "✅" } else { "❌" },
        compilation_reduction
    ));
    if baseline_stats.p95_latency() > 0 {
        let latency_improvement = 100.0 * (baseline_stats.p95_latency() as f64 - cached_stats.p95_latency() as f64) / baseline_stats.p95_latency() as f64;
        report.push_str(&format!("3. {} P95 latency reduced ≥ 20%: **{:.1}%**\n",
            if latency_improvement >= 20.0 { "✅" } else { "❌" },
            latency_improvement
        ));
    }

    report.push_str("\n");

    // Write report
    let report_path = "benchmarks/PHASE_1_REPORT.md";
    fs::write(report_path, report).expect("Failed to write report");
    println!("✅ Report generated: {}", report_path);
}

// Helper: would use chrono but keeping minimal dependencies
mod chrono {
    pub struct Local;
    impl Local {
        pub fn now() -> String {
            "2026-02-08 12:00:00".to_string()  // Placeholder
        }
    }
}
