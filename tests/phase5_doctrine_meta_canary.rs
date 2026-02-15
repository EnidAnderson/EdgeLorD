//! Phase 5 Doctrine Metadata Canary Suite for Satellites
//!
//! This test verifies that the satellites crate correctly reads doctrine metadata
//! emitted by Tier 3 stdlib expansion. All four golden vectors must pass before shipping.
//!
//! **Contract**: IMMUTABLE (locked in PHASE_5_SATELLITE_CONTRACT.md)
//! **Status**: CANARY SUITE FOR SATELLITES CRATE

use tcb_core::phase5_doctrine_meta::{read_doctrine_from_quoted_meta, DoctrineMetadata};
use tcb_core::bundle_canonical::{CoreSExpr, CoreAtom};

// ============================================================================
// GOLDEN VECTOR TEST SUITE - All four must pass for CI gate
// ============================================================================

/// Golden Vector 1: Singleton with both NF and legacy keys
/// Expected: Both keys present, reads as Stack([linear])
#[test]
fn satellites_canary_singleton_both_keys() {
    // Simulate doctrine metadata from (with-doctrine 'linear BODY)
    let meta = create_singleton_meta_both_keys();

    let result = read_doctrine_from_quoted_meta(&meta);
    assert!(result.is_ok(), "Should parse singleton with both keys");

    let doc = result.unwrap();
    assert_eq!(doc.stack_size(), 1);
    assert_eq!(doc.stack[0], "linear");
    assert!(doc.has_nf_key, "Should have NF key");
    assert!(doc.has_legacy_key, "Should have legacy key for singleton");

    // Verify conformance
    assert!(doc.verify_conformance().is_ok(),
            "Singleton with both keys must pass conformance");
}

/// Golden Vector 2: Multi-element NF only (NO legacy key)
/// Expected: Stack([A,B,C]), only NF key, no legacy key
/// CRITICAL: This is backwards compat enforcement
#[test]
fn satellites_canary_multi_element_nf_only() {
    // Simulate doctrine metadata from (with-doctrine-stack (A B C) BODY)
    let meta = create_multi_element_meta_nf_only();

    let result = read_doctrine_from_quoted_meta(&meta);
    assert!(result.is_ok(), "Should parse multi-element stack");

    let doc = result.unwrap();
    assert_eq!(doc.stack_size(), 3);
    assert_eq!(doc.stack, vec!["A", "B", "C"]);
    assert!(doc.has_nf_key, "Should have NF key");
    assert!(!doc.has_legacy_key, "Multi-element must NOT have legacy key (backwards compat!)");

    // Verify conformance
    assert!(doc.verify_conformance().is_ok(),
            "Multi-element without legacy key must pass conformance");
}

/// Golden Vector 3: Version v2 forward compatibility
/// Expected: Accepts v2, same structure as v1
#[test]
fn satellites_canary_doctrine_v2_forward_compat() {
    // Simulate future v2 doctrine (same structure, different version tag)
    let meta = create_v2_meta();

    let result = read_doctrine_from_quoted_meta(&meta);
    assert!(result.is_ok(), "Must accept v2 gracefully (forward compat)");

    let doc = result.unwrap();
    assert_eq!(doc.version, "v2");
    assert_eq!(doc.stack, vec!["X", "Y"]);
    assert!(doc.has_nf_key);

    // Verify conformance
    assert!(doc.verify_conformance().is_ok(),
            "v2 metadata must pass conformance");
}

/// Golden Vector 4: Empty stack (edge case)
/// Expected: Stack([]) is valid
#[test]
fn satellites_canary_empty_doctrine_stack() {
    // Simulate empty doctrine (valid edge case)
    let meta = create_empty_stack_meta();

    let result = read_doctrine_from_quoted_meta(&meta);
    assert!(result.is_ok(), "Should accept empty stack as valid");

    let doc = result.unwrap();
    assert_eq!(doc.stack_size(), 0);
    assert_eq!(doc.stack.len(), 0);

    // Verify conformance
    assert!(doc.verify_conformance().is_ok(),
            "Empty stack must pass conformance");
}

// ============================================================================
// MASTER CANARY TEST - Exercises all four vectors
// ============================================================================

/// Master test combining all four vectors
/// This is the test satellites must pass before shipping to production
#[test]
fn master_satellites_doctrine_meta_conformance() {
    // Test V1: Singleton
    {
        let meta = create_singleton_meta_both_keys();
        let doc = read_doctrine_from_quoted_meta(&meta).unwrap();
        assert_eq!(doc.stack_size(), 1);
        assert!(doc.verify_conformance().is_ok());
    }

    // Test V2: Multi-element (critical backwards compat)
    {
        let meta = create_multi_element_meta_nf_only();
        let doc = read_doctrine_from_quoted_meta(&meta).unwrap();
        assert_eq!(doc.stack_size(), 3);
        assert!(!doc.has_legacy_key, "Backwards compat: no legacy key for multi");
        assert!(doc.verify_conformance().is_ok());
    }

    // Test V3: v2 forward compat
    {
        let meta = create_v2_meta();
        let doc = read_doctrine_from_quoted_meta(&meta).unwrap();
        assert_eq!(doc.version, "v2");
        assert!(doc.verify_conformance().is_ok());
    }

    // Test V4: Empty stack
    {
        let meta = create_empty_stack_meta();
        let doc = read_doctrine_from_quoted_meta(&meta).unwrap();
        assert_eq!(doc.stack_size(), 0);
        assert!(doc.verify_conformance().is_ok());
    }

    println!("✅ All four golden vectors passed");
    println!("✅ Satellites conformance verified");
    println!("✅ Ready for production deployment");
}

// ============================================================================
// NEGATIVE TESTS - Verify contract enforcement
// ============================================================================

/// Negative test: Multi-element with legacy key should fail conformance
#[test]
fn satellites_negative_multi_with_legacy_violates_contract() {
    // Construct a violating form with BOTH NF and legacy keys for multi-element
    let violating = CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
        CoreSExpr::Atom(CoreAtom::Symbol("meta".to_string())),
        // NF key (valid)
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine-nf".to_string())),
            CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
                CoreSExpr::Atom(CoreAtom::Symbol("doctrine".to_string())),
                CoreSExpr::Atom(CoreAtom::Symbol("v1".to_string())),
                CoreSExpr::List(vec![
                    CoreSExpr::Atom(CoreAtom::Symbol("stack".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("A".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("B".to_string())),
                ]),
            ]))),
        ]),
        // Legacy key (INVALID for multi-element) - the violation
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine".to_string())),
            CoreSExpr::Atom(CoreAtom::Symbol("violation".to_string())),
        ]),
    ])));

    let doc = read_doctrine_from_quoted_meta(&violating).unwrap();

    // This MUST fail conformance check
    assert!(doc.verify_conformance().is_err(),
            "Multi-element with legacy key must FAIL conformance (backwards compat violation)");
}

// ============================================================================
// HELPER FUNCTIONS - Meta form constructors
// ============================================================================

fn create_singleton_meta_both_keys() -> CoreSExpr {
    // (meta (ambient-doctrine-nf '(doctrine v1 (stack linear)))
    //       (ambient-doctrine linear))
    CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
        CoreSExpr::Atom(CoreAtom::Symbol("meta".to_string())),
        // NF key
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine-nf".to_string())),
            CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
                CoreSExpr::Atom(CoreAtom::Symbol("doctrine".to_string())),
                CoreSExpr::Atom(CoreAtom::Symbol("v1".to_string())),
                CoreSExpr::List(vec![
                    CoreSExpr::Atom(CoreAtom::Symbol("stack".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("linear".to_string())),
                ]),
            ]))),
        ]),
        // Legacy key
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine".to_string())),
            CoreSExpr::Atom(CoreAtom::Symbol("linear".to_string())),
        ]),
    ])))
}

fn create_multi_element_meta_nf_only() -> CoreSExpr {
    // (meta (ambient-doctrine-nf '(doctrine v1 (stack A B C))))
    // NO legacy key
    CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
        CoreSExpr::Atom(CoreAtom::Symbol("meta".to_string())),
        // NF key only
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine-nf".to_string())),
            CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
                CoreSExpr::Atom(CoreAtom::Symbol("doctrine".to_string())),
                CoreSExpr::Atom(CoreAtom::Symbol("v1".to_string())),
                CoreSExpr::List(vec![
                    CoreSExpr::Atom(CoreAtom::Symbol("stack".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("A".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("B".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("C".to_string())),
                ]),
            ]))),
        ]),
    ])))
}

fn create_v2_meta() -> CoreSExpr {
    // (meta (ambient-doctrine-nf '(doctrine v2 (stack X Y))))
    CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
        CoreSExpr::Atom(CoreAtom::Symbol("meta".to_string())),
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine-nf".to_string())),
            CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
                CoreSExpr::Atom(CoreAtom::Symbol("doctrine".to_string())),
                CoreSExpr::Atom(CoreAtom::Symbol("v2".to_string())),
                CoreSExpr::List(vec![
                    CoreSExpr::Atom(CoreAtom::Symbol("stack".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("X".to_string())),
                    CoreSExpr::Atom(CoreAtom::Symbol("Y".to_string())),
                ]),
            ]))),
        ]),
    ])))
}

fn create_empty_stack_meta() -> CoreSExpr {
    // (meta (ambient-doctrine-nf '(doctrine v1 (stack))))
    CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
        CoreSExpr::Atom(CoreAtom::Symbol("meta".to_string())),
        CoreSExpr::List(vec![
            CoreSExpr::Atom(CoreAtom::Symbol("ambient-doctrine-nf".to_string())),
            CoreSExpr::Quote(Box::new(CoreSExpr::List(vec![
                CoreSExpr::Atom(CoreAtom::Symbol("doctrine".to_string())),
                CoreSExpr::Atom(CoreAtom::Symbol("v1".to_string())),
                CoreSExpr::List(vec![
                    CoreSExpr::Atom(CoreAtom::Symbol("stack".to_string())),
                ]),
            ]))),
        ]),
    ])))
}
