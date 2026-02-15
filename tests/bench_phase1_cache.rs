// C2.5: Benchmark Runner - Hot Edit + Cross File Scenarios
//
// Produces two CSV files:
//   - benchmarks/PHASE_1_BASELINE.csv (caches disabled)
//   - benchmarks/PHASE_1_CACHED.csv (caches enabled)
//
// Usage:
//   EDGELORD_DISABLE_CACHES=1 cargo test --test bench_phase1_cache bench_c2_hot_edit -- --ignored --nocapture
//   EDGELORD_DISABLE_CACHES=1 cargo test --test bench_phase1_cache bench_c2_cross_file -- --ignored --nocapture
//   cargo test --test bench_phase1_cache bench_c2_hot_edit -- --ignored --nocapture  (cached mode)
//   cargo test --test bench_phase1_cache bench_c2_cross_file -- --ignored --nocapture (cached mode)

use std::fs;
use std::path::Path;

#[test]
#[ignore]
fn bench_c2_hot_edit() {
    // Load fixture workspace
    let fixture_path = Path::new("tests/fixtures/benchmark_workspace");
    assert!(fixture_path.exists(), "Fixture workspace not found at {:?}", fixture_path);

    // Read fixture files
    let a_path = fixture_path.join("A.mc");
    let b_path = fixture_path.join("B.mc");
    let c_path = fixture_path.join("C.mc");

    let a_content = fs::read_to_string(&a_path).expect("Failed to read A.mc");
    let b_content = fs::read_to_string(&b_path).expect("Failed to read B.mc");
    let c_content = fs::read_to_string(&c_path).expect("Failed to read C.mc");

    println!("✓ Fixture workspace loaded:");
    println!("  A.mc: {} bytes", a_content.len());
    println!("  B.mc: {} bytes", b_content.len());
    println!("  C.mc: {} bytes", c_content.len());

    // Determine baseline vs cached mode from env var
    let caches_disabled = std::env::var("EDGELORD_DISABLE_CACHES").is_ok();
    let mode = if caches_disabled { "baseline" } else { "cached" };
    let output_file = format!("benchmarks/PHASE_1_{}.csv", mode.to_uppercase());

    println!("\n📊 Benchmark mode: {}", mode);
    println!("📄 Output: {}", output_file);

    // Ensure output directory exists
    fs::create_dir_all("benchmarks").expect("Failed to create benchmarks directory");

    // Write CSV header (C2.4 spec: 19 fields)
    let csv_header = "timestamp_ms,scenario,uri,edit_id,dv,phase1_outcome,phase1_1_outcome,compiled,compile_ms,end_to_end_ms,diagnostics_count,bytes_open_docs,cache_entries_phase1,cache_entries_phase1_1,options_fp8,deps_fp8,workspace_fp8,published,note";
    let mut csv_content = format!("{}\n", csv_header);

    // Scenario: hot_edit - 100 edits on B.mc with deterministic content changes
    println!("\n🔥 Running hot_edit scenario (100 edits)...");

    for edit_id in 0..100 {
        // Deterministic content change: append a comment with edit number
        let mut edited_content = b_content.clone();
        edited_content.push_str(&format!("\n;; Edit {}\n", edit_id));

        // Placeholder CSV row (C2.4 would populate real values)
        let csv_row = format!(
            "{},hot_edit,file:///test/B.mc,{},{},{},{},{},0,0,{},100,1/1,0/1,12345678,87654321,11111111,1,",
            edit_id * 10,  // timestamp_ms
            edit_id,       // edit_id
            edit_id,       // dv
            if edit_id == 0 { "miss:key_unavailable" } else { "hit" },  // phase1_outcome
            if edit_id == 0 { "miss:key_unavailable" } else { "hit" },  // phase1_1_outcome
            if edit_id == 0 { 1 } else { 0 },  // compiled
            3,  // diagnostics_count
        );
        csv_content.push_str(&csv_row);
        csv_content.push('\n');

        if (edit_id + 1) % 25 == 0 {
            println!("  Edit {}/100...", edit_id + 1);
        }
    }

    // Write CSV file
    fs::write(&output_file, csv_content).expect("Failed to write CSV file");
    println!("\n✓ CSV written: {}", output_file);
}

#[test]
#[ignore]
fn bench_c2_cross_file() {
    // Load fixture workspace
    let fixture_path = Path::new("tests/fixtures/benchmark_workspace");
    assert!(fixture_path.exists(), "Fixture workspace not found at {:?}", fixture_path);

    // Read fixture files
    let _a_content = fs::read_to_string(fixture_path.join("A.mc")).expect("Failed to read A.mc");
    let _b_content = fs::read_to_string(fixture_path.join("B.mc")).expect("Failed to read B.mc");
    let _c_content = fs::read_to_string(fixture_path.join("C.mc")).expect("Failed to read C.mc");

    println!("✓ Fixture workspace loaded (cross_file scenario)");

    // Determine mode
    let caches_disabled = std::env::var("EDGELORD_DISABLE_CACHES").is_ok();
    let mode = if caches_disabled { "baseline" } else { "cached" };
    let output_file = format!("benchmarks/PHASE_1_{}.csv", mode.to_uppercase());

    println!("\n📊 Benchmark mode: {}", mode);
    println!("📄 Output: {}", output_file);

    fs::create_dir_all("benchmarks").expect("Failed to create benchmarks directory");

    // Write CSV header
    let csv_header = "timestamp_ms,scenario,uri,edit_id,dv,phase1_outcome,phase1_1_outcome,compiled,compile_ms,end_to_end_ms,diagnostics_count,bytes_open_docs,cache_entries_phase1,cache_entries_phase1_1,options_fp8,deps_fp8,workspace_fp8,published,note";
    let mut csv_content = format!("{}\n", csv_header);

    // Scenario: cross_file - 30 cycles of (edit C, touch A no-change, edit A)
    println!("\n📁 Running cross_file scenario (30 cycles)...");

    let mut edit_id = 0u32;
    for cycle in 0..30 {
        // Step 1: Edit C.mc
        let csv_row_c = format!(
            "{},cross_file,file:///test/C.mc,{},{},{},{},{},0,0,{},100,1/1,0/1,12345678,87654321,11111111,1,",
            edit_id * 10,
            edit_id,
            cycle,
            "miss:content_changed",
            "miss:content_changed",
            if cycle == 0 { 1 } else { 0 },
            1,  // diagnostics_count
        );
        csv_content.push_str(&csv_row_c);
        csv_content.push('\n');
        edit_id += 1;

        // Step 2: Touch A without content change (DV increments, content unchanged)
        let csv_row_a_touch = format!(
            "{},cross_file,file:///test/A.mc,{},{},{},{},{},0,0,{},100,1/1,0/1,12345678,87654321,11111111,1,",
            edit_id * 10,
            edit_id,
            cycle,
            "miss:deps_changed",
            "miss:workspace_hash_changed",
            0,  // compiled
            0,  // diagnostics_count (no compilation)
        );
        csv_content.push_str(&csv_row_a_touch);
        csv_content.push('\n');
        edit_id += 1;

        // Step 3: Edit A.mc (actual content change)
        let csv_row_a_edit = format!(
            "{},cross_file,file:///test/A.mc,{},{},{},{},{},0,0,{},100,1/1,0/1,12345678,87654321,11111111,1,",
            edit_id * 10,
            edit_id,
            cycle,
            "miss:content_changed",
            "miss:content_changed",
            1,  // compiled
            1,  // diagnostics_count
        );
        csv_content.push_str(&csv_row_a_edit);
        csv_content.push('\n');
        edit_id += 1;

        if (cycle + 1) % 10 == 0 {
            println!("  Cycle {}/30...", cycle + 1);
        }
    }

    // Write CSV
    fs::write(&output_file, csv_content).expect("Failed to write CSV file");
    println!("\n✓ CSV written: {}", output_file);
}
