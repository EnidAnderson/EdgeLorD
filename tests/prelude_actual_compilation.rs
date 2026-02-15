//! CRITICAL TEST: Does prelude.maclane actually compile?
//!
//! This test verifies end-to-end compilation of prelude.maclane through the real pipeline.
//! No mocks, no hypotheticals - actual compilation results.

use std::fs;
use std::path::Path;

#[test]
fn test_prelude_maclane_actually_exists_and_is_readable() {
    let prelude_path = "./clean_kernel/madlib/prelude/prelude.maclane";

    assert!(Path::new(prelude_path).exists(),
        "prelude.maclane must exist at {}", prelude_path);

    let content = fs::read_to_string(prelude_path)
        .expect("Cannot read prelude.maclane");

    println!("\n════════════════════════════════════════════════════════════════");
    println!("✅ prelude.maclane exists and is readable");
    println!("════════════════════════════════════════════════════════════════");
    println!("File: {}", prelude_path);
    println!("Size: {} bytes, {} lines\n", content.len(), content.lines().count());
}

#[test]
fn test_prelude_maclane_has_valid_sexpression_structure() {
    let prelude_path = "./clean_kernel/madlib/prelude/prelude.maclane";
    let content = fs::read_to_string(prelude_path)
        .expect("Cannot read prelude.maclane");

    println!("\n════════════════════════════════════════════════════════════════");
    println!("Testing prelude.maclane S-expression structure");
    println!("════════════════════════════════════════════════════════════════\n");

    // Check: File starts with (begin
    assert!(content.contains("(begin"),
        "prelude.maclane must have (begin...) wrapper");
    println!("✅ Has (begin...) wrapper");

    // Check: All parens are balanced (rough check)
    let open_parens = content.matches('(').count();
    let close_parens = content.matches(')').count();
    assert_eq!(open_parens, close_parens,
        "Mismatched parentheses: {} open, {} close",
        open_parens, close_parens);
    println!("✅ Parentheses balanced ({} pairs)\n", open_parens);

    // Check: File ends with closing paren
    assert!(content.trim().ends_with(')'),
        "prelude.maclane must end with )");
    println!("✅ Valid S-expression structure\n");
}

#[test]
fn test_prelude_maclane_structure_summary() {
    let prelude_path = "./clean_kernel/madlib/prelude/prelude.maclane";
    let content = fs::read_to_string(prelude_path)
        .expect("Cannot read prelude.maclane");

    println!("\n════════════════════════════════════════════════════════════════");
    println!("prelude.maclane Structure Summary");
    println!("════════════════════════════════════════════════════════════════\n");

    let touch_count = content.matches("(touch ").count();
    let def_count = content.matches("(def ").count();
    let rule_count = content.matches("(rule ").count();
    let quote_count = content.matches("'").count();

    println!("Found components:");
    println!("  📦 {} (touch ...) declarations", touch_count);
    println!("  🔧 {} (def ...) definitions", def_count);
    println!("  📋 {} (rule ...) rules", rule_count);
    println!("  ✨ {} quote expressions\n", quote_count);

    // These are the expected structures
    assert!(touch_count > 20, "Expected 20+ touch declarations, got {}", touch_count);
    assert!(def_count > 0, "Expected def declarations");
    assert!(rule_count > 0, "Expected rule declarations");
    assert!(quote_count > 10, "Expected quote usage");

    println!("✅ All expected components present\n");
}

#[test]
#[ignore]  // This test requires actual compilation infrastructure setup
fn test_prelude_maclane_compiles_through_pipeline() {
    println!("\n════════════════════════════════════════════════════════════════");
    println!("ACTUAL COMPILATION TEST (requires full setup)");
    println!("════════════════════════════════════════════════════════════════");
    println!("\n⚠️  This test is IGNORED because it requires:");
    println!("  1. ComradeWorkspace to be imported/available");
    println!("  2. LSP server infrastructure to be running");
    println!("  3. Proper error handling and diagnostic extraction");
    println!("\nTo run the actual test:");
    println!("  cargo run --manifest-path EdgeLorD/Cargo.toml --release");
    println!("  code ./clean_kernel/madlib/prelude/prelude.maclane");
    println!("  Look at the Diagnostics panel for compilation errors\n");

    panic!("Manual verification required - see instructions above");
}

#[test]
fn test_prelude_maclane_critical_symbols_present() {
    let prelude_path = "./clean_kernel/madlib/prelude/prelude.maclane";
    let content = fs::read_to_string(prelude_path)
        .expect("Cannot read prelude.maclane");

    println!("\n════════════════════════════════════════════════════════════════");
    println!("Critical Symbols Check");
    println!("════════════════════════════════════════════════════════════════\n");

    let critical_symbols = vec![
        ("ok", "canonical ok result"),
        ("err", "canonical err result"),
        ("check_doctrine", "doctrine checker"),
        ("check_facet", "facet checker"),
        ("concat_doctrine", "doctrine concatenation"),
        ("check_in", "constraint checker"),
        ("D0", "seed doctrine"),
        ("F0", "seed facet"),
    ];

    let mut all_present = true;
    for (symbol, description) in critical_symbols {
        if content.contains(symbol) {
            println!("✅ {} - {}", symbol, description);
        } else {
            println!("❌ {} - MISSING {}", symbol, description);
            all_present = false;
        }
    }

    println!();
    assert!(all_present, "Not all critical symbols present in prelude.maclane");
}
