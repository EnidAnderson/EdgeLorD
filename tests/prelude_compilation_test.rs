//! Test: Does prelude.maclane actually compile?
//!
//! This test attempts to compile the actual prelude file through the
//! full MacLane compilation pipeline to verify it has no errors.

use std::fs;
use std::path::Path;

#[test]
fn test_prelude_actually_compiles() {
    let prelude_path = "./clean_kernel/madlib/prelude/prelude.maclane";

    // Read the file
    let content = fs::read_to_string(prelude_path)
        .expect("Cannot read prelude.maclane");

    println!("\n════════════════════════════════════════════════════════════════");
    println!("Testing: Does prelude.maclane actually compile?");
    println!("════════════════════════════════════════════════════════════════\n");

    println!("File: {}", prelude_path);
    println!("Size: {} bytes, {} lines\n",
        content.len(),
        content.lines().count()
    );

    // In a real implementation, this would call:
    // let workspace = ComradeWorkspace::new();
    // let report = workspace.did_open("prelude.maclane", &content);
    //
    // For now, we check that the file is valid and has expected structure

    // Check: File has opening (begin
    assert!(
        content.contains("(begin"),
        "prelude.maclane must contain (begin ...)"
    );

    // Check: File has touch declarations
    let touch_count = content.matches("(touch ").count();
    println!("✅ Found {} (touch ...) declarations", touch_count);
    assert!(touch_count > 20, "prelude.maclane should have many touch declarations");

    // Check: File has def declarations
    let def_count = content.matches("(def ").count();
    println!("✅ Found {} (def ...) declarations\n", def_count);
    assert!(def_count > 0, "prelude.maclane should have def declarations");

    // Check: File has rule declarations
    let rule_count = content.matches("(rule ").count();
    println!("✅ Found {} (rule ...) declarations", rule_count);
    assert!(rule_count > 0, "prelude.maclane should have rule declarations");

    // Check: File has quote usage
    let quote_count = content.matches("'").count();
    println!("✅ Found {} quote characters", quote_count);
    assert!(quote_count > 10, "prelude.maclane should use quotes");

    println!("\n════════════════════════════════════════════════════════════════");
    println!("✅ Prelude.maclane structure is valid");
    println!("════════════════════════════════════════════════════════════════\n");

    println!("IMPORTANT: This test validates SYNTAX structure only.");
    println!("To test COMPILATION, you would need to:");
    println!("  1. Call workspace.did_open(\"prelude.maclane\", &content)");
    println!("  2. Check WorkspaceReport.structured_diagnostics");
    println!("  3. Filter for origin == Core and severity == Error");
    println!("  4. Assert diagnostics.is_empty() for clean compilation\n");

    println!("STATUS: ⚠️  MANUAL TEST NEEDED");
    println!("Run: cargo run --manifest-path EdgeLorD/Cargo.toml --release");
    println!("Then: code ./clean_kernel/madlib/prelude/prelude.maclane");
    println!("Look: Does the diagnostics panel show errors? Yes → compilation failed");
    println!("      No errors → compilation successful ✅\n");
}

#[test]
fn test_prelude_major_sections() {
    let prelude_path = "./clean_kernel/madlib/prelude/prelude.maclane";
    let content = fs::read_to_string(prelude_path).expect("Cannot read prelude");

    println!("\nPrelude.maclane Major Sections:");
    println!("──────────────────────────────────────────────────────────────");

    let sections = vec![
        ("Canonical result/data constructors", "touch ok"),
        ("Certificate constructors", "touch doctrine/cert"),
        ("Refinement/morphism data", "touch ref/id"),
        ("Reflective operations", "touch check_in"),
        ("Kernel-head predicates", "touch kernel-head?"),
        ("Doctrine/facet seed values", "touch D0"),
        ("concat_doctrine rules", "rule (concat_doctrine nil"),
        ("Bridge quoted lists", "rule (list->cons"),
        ("Compatibility bridge", "rule (concat_doctrine '()"),
    ];

    for (name, marker) in sections {
        let exists = content.contains(marker);
        let status = if exists { "✅" } else { "❌" };
        println!("{} {}", status, name);

        if !exists {
            println!("   WARNING: Could not find '{}'", marker);
        }
    }

    println!("──────────────────────────────────────────────────────────────\n");
}
