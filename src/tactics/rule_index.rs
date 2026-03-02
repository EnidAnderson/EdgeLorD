//! SC0: Doctrine-Aware Rule Discovery
//!
//! `RuleIndex` provides fast lookup of applicable rewrite rules for a given
//! proof goal.  It is built from a `CoreBundleV0` and cached per document.
//!
//! **INV S-RULE-QUERY**: Deterministic — identical inputs produce identical
//! output lists (BTreeMap + sort-stable).
//! **INV D-***: No hash map iteration over output-affecting state.

use comrade_lisp::core::{CompiledRule, CoreBundleV0};
use comrade_lisp::proof_state::GoalState;
use std::collections::BTreeMap;

// ─── Match confidence ─────────────────────────────────────────────────────────

/// How confidently a rule applies to a goal.
///
/// Ordered from most to least specific.  Used to sort `RuleMatch` results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchConfidence {
    /// Rule name exactly equals goal name.
    Exact,
    /// Rule LHS structurally unifies with the goal's expected morphism term
    /// (heuristic check until SC1 adds full pattern matching).
    Unifiable,
    /// Head symbol of rule LHS matches goal name prefix.
    HeadMatch,
    /// Only the doctrine context matches; no direct structural evidence.
    DoctrineOnly,
}

/// A candidate rule for a goal, with match metadata.
#[derive(Debug, Clone)]
pub struct RuleMatch<'a> {
    pub rule: &'a CompiledRule,
    pub confidence: MatchConfidence,
    /// Whether this rule is gated behind a witness obligation.
    pub needs_witness: bool,
}

// ─── Rule index ───────────────────────────────────────────────────────────────

/// Searchable index over the rules in a `CoreBundleV0`.
pub struct RuleIndex {
    rules: Vec<CompiledRule>,
    /// Doctrine name → sorted rule indices (INV D-*)
    by_doctrine: BTreeMap<String, Vec<usize>>,
    /// Indices of rules with no doctrine
    no_doctrine: Vec<usize>,
}

impl RuleIndex {
    /// Build from a compiled bundle.  O(n) in rule count.
    pub fn build(bundle: &CoreBundleV0) -> Self {
        let rules = bundle.rules.clone();
        let mut by_doctrine: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut no_doctrine = Vec::new();
        for (i, rule) in rules.iter().enumerate() {
            match &rule.doctrine_context {
                Some(d) => by_doctrine.entry(d.clone()).or_default().push(i),
                None => no_doctrine.push(i),
            }
        }
        Self { rules, by_doctrine, no_doctrine }
    }

    pub fn is_empty(&self) -> bool { self.rules.is_empty() }
    pub fn len(&self) -> usize { self.rules.len() }

    /// All rules belonging to `doctrine`, or rules with no doctrine if `None`.
    pub fn rules_for_doctrine(&self, doctrine: Option<&str>) -> Vec<&CompiledRule> {
        match doctrine {
            Some(d) => self.by_doctrine
                .get(d)
                .map(|idxs| idxs.iter().map(|&i| &self.rules[i]).collect())
                .unwrap_or_default(),
            None => self.no_doctrine.iter().map(|&i| &self.rules[i]).collect(),
        }
    }

    /// All rules that could apply to `goal`, ordered by confidence (best first).
    ///
    /// **INV D-***: Sorted by `(confidence, name)` — deterministic.
    pub fn rules_matching_goal<'a>(&'a self, goal: &GoalState) -> Vec<RuleMatch<'a>> {
        let mut matches = Vec::new();
        for rule in &self.rules {
            let confidence = if rule.name == goal.name {
                MatchConfidence::Exact
            } else {
                let head = extract_head_name(&rule.lhs);
                let head_matches = head.as_deref()
                    .map(|h| {
                        goal.name.contains(h)
                            || h.contains(goal.name.as_str())
                            || normalize(h) == normalize(&goal.name)
                    })
                    .unwrap_or(false);
                if head_matches {
                    MatchConfidence::HeadMatch
                } else if rule.doctrine_context.is_some() {
                    MatchConfidence::DoctrineOnly
                } else {
                    continue; // rule has no structural or doctrine relevance
                }
            };
            let needs_witness = is_witness_gated(&rule.name);
            matches.push(RuleMatch { rule, confidence, needs_witness });
        }
        // INV D-*: stable sort
        matches.sort_by(|a, b| {
            a.confidence.cmp(&b.confidence)
                .then(a.rule.name.cmp(&b.rule.name))
        });
        matches
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

const WITNESS_GATE_NAMES: &[&str] = &[
    "bc-check", "frobenius-check", "descent-check", "flat-witness",
];

fn is_witness_gated(rule_name: &str) -> bool {
    WITNESS_GATE_NAMES.iter().any(|&g| rule_name.contains(g))
}

fn extract_head_name(term: &tcb_core::ast::MorphismTerm) -> Option<String> {
    use tcb_core::ast::MorphismTerm as MT;
    match term {
        MT::Generator { id, .. } => Some(format!("{id:?}")),
        MT::App { op, .. }       => Some(format!("{op:?}")),
        MT::InDoctrine { term, .. } => extract_head_name(term),
        MT::Compose { components, .. } => {
            components.first().and_then(|c| extract_head_name(c))
        }
        _ => None,
    }
}

fn normalize(s: &str) -> String {
    s.to_lowercase().replace('-', "_").replace('/', "_")
}
