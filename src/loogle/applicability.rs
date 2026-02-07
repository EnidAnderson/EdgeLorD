/// Applicability engine that checks if a lemma can unify with a goal
use crate::proposal::{Proposal, ProposalKind, ProposalStatus, EvidenceSummary, ReconstructionPlan};
use super::LoogleResult;
use std::collections::HashMap;

/// Result of checking applicability of a lemma to a goal
#[derive(Debug, Clone)]
pub struct ApplicabilityResult {
    pub applicable: bool,
    pub confidence: f32,  // 0.0 to 1.0
    pub unification_preview: Option<String>,  // e.g., "?A := Nat, ?B := List Nat"
    pub pedagogical_rationale: String,
}

/// Check if a lemma result is applicable to a current goal
pub fn check_applicability(
    lemma: &LoogleResult,
    goal_fingerprint: &str,
) -> ApplicabilityResult {
    // Parse fingerprints to structural representation
    let lemma_structure = parse_fingerprint(&lemma.rationale);
    let goal_structure = parse_fingerprint(goal_fingerprint);
    
    // Attempt unification
    match unify_structures(&lemma_structure, &goal_structure) {
        Some(substitutions) => {
            let confidence = compute_confidence(&substitutions);
            let preview = format_substitutions(&substitutions);
            
            let pedagogical_rationale = format!(
                "Lemma '{}' applies because its structure matches your goal{}. {}",
                lemma.name,
                if substitutions.is_empty() { "" } else { " with substitutions" },
                lemma.doc
            );
            
            ApplicabilityResult {
                applicable: true,
                confidence,
                unification_preview: Some(preview),
                pedagogical_rationale,
            }
        }
        None => {
            // Fall back to substring matching for partial relevance
            let partial_match = lemma.rationale.contains(goal_fingerprint) 
                || goal_fingerprint.contains(&lemma.name);
            
            let confidence = if partial_match { 0.3 } else { 0.0 };
            
            ApplicabilityResult {
                applicable: partial_match,
                confidence,
                unification_preview: None,
                pedagogical_rationale: format!(
                    "Lemma '{}' has {} relevance to this goal",
                    lemma.name,
                    if partial_match { "partial" } else { "no" }
                ),
            }
        }
    }
}

/// Lightweight structural representation for unification
#[derive(Debug, Clone, PartialEq)]
enum TermStructure {
    Gen { index: String, arity: String },
    Compose(Vec<TermStructure>),
    App { op: String, args: Vec<TermStructure> },
    Hole(String),
    Doctrine { scope: String, inner: Box<TermStructure> },
    Reject(String),
    Other(String),
}

/// Parse a fingerprint string into a TermStructure
fn parse_fingerprint(fp: &str) -> TermStructure {
    let fp = fp.trim();
    
    // Parse hole pattern: ?name
    if fp.starts_with('?') {
        return TermStructure::Hole(fp[1..].to_string());
    }
    
    // Parse rejection: !code
    if fp.starts_with('!') {
        return TermStructure::Reject(fp[1..].to_string());
    }
    
    // Parse generator: gen:index:[in→out]
    if fp.starts_with("gen:") {
        let parts: Vec<&str> = fp[4..].split(':').collect();
        return TermStructure::Gen {
            index: parts.get(0).unwrap_or(&"").to_string(),
            arity: parts.get(1).unwrap_or(&"").to_string(),
        };
    }
    
    // Parse composition: comp:[parts]:[in→out]
    if fp.starts_with("comp:[") {
        if let Some(end) = fp.find("]:[") {
            let inner = &fp[6..end];
            let components: Vec<TermStructure> = inner
                .split(';')
                .filter(|s| !s.is_empty())
                .map(parse_fingerprint)
                .collect();
            return TermStructure::Compose(components);
        }
    }
    
    // Parse application: app:op:idx(args):[in→out]
    if fp.starts_with("app:") {
        // Find the opening paren
        if let Some(paren_start) = fp.find('(') {
            let op_part = &fp[4..paren_start];
            if let Some(paren_end) = fp.rfind("):") {
                let args_str = &fp[paren_start + 1..paren_end];
                let args: Vec<TermStructure> = args_str
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(parse_fingerprint)
                    .collect();
                return TermStructure::App {
                    op: op_part.to_string(),
                    args,
                };
            }
        }
    }
    
    // Parse doctrine wrapper: doc:scope:inner
    if fp.starts_with("doc:") {
        let rest = &fp[4..];
        if let Some(colon_pos) = rest.find(':') {
            let scope = &rest[..colon_pos];
            let inner = parse_fingerprint(&rest[colon_pos + 1..]);
            return TermStructure::Doctrine {
                scope: scope.to_string(),
                inner: Box::new(inner),
            };
        }
    }
    
    // Fallback: treat as opaque string
    TermStructure::Other(fp.to_string())
}

/// Simple first-order unification
fn unify_structures(
    pattern: &TermStructure,
    term: &TermStructure,
) -> Option<HashMap<String, TermStructure>> {
    let mut subst = HashMap::new();
    if unify_inner(pattern, term, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

/// Recursive unification helper
fn unify_inner(
    pattern: &TermStructure,
    term: &TermStructure,
    subst: &mut HashMap<String, TermStructure>,
) -> bool {
    match (pattern, term) {
        // Hole matches anything
        (TermStructure::Hole(name), _) => {
            if let Some(existing) = subst.get(name) {
                // Check consistency
                existing == term
            } else {
                subst.insert(name.clone(), term.clone());
                true
            }
        }
        
        // Same structure?
        (TermStructure::Gen { index: i1, arity: a1 }, TermStructure::Gen { index: i2, arity: a2 }) => {
            i1 == i2 && a1 == a2
        }
        
        (TermStructure::Compose(ps), TermStructure::Compose(ts)) => {
            ps.len() == ts.len() && ps.iter().zip(ts).all(|(p, t)| unify_inner(p, t, subst))
        }
        
        (TermStructure::App { op: op1, args: args1 }, TermStructure::App { op: op2, args: args2 }) => {
            op1 == op2 && args1.len() == args2.len() 
                && args1.iter().zip(args2).all(|(a1, a2)| unify_inner(a1, a2, subst))
        }
        
        (TermStructure::Doctrine { scope: s1, inner: i1 }, TermStructure::Doctrine { scope: s2, inner: i2 }) => {
            s1 == s2 && unify_inner(i1, i2, subst)
        }
        
        (TermStructure::Reject(c1), TermStructure::Reject(c2)) => c1 == c2,
        
        (TermStructure::Other(s1), TermStructure::Other(s2)) => s1 == s2,
        
        // Different structure types don't unify
        _ => false,
    }
}

/// Compute confidence based on substitution complexity
fn compute_confidence(subst: &HashMap<String, TermStructure>) -> f32 {
    // Base confidence for successful unification
    let base = 0.9;
    
    // Penalty per substitution (simpler is better)
    let penalty_per_binding = 0.05;
    
    // Penalty for complex substitutions
    let complexity_penalty: f32 = subst.values()
        .map(|v| structure_complexity(v) as f32 * 0.02)
        .sum();
    
    (base - (subst.len() as f32 * penalty_per_binding) - complexity_penalty).max(0.5)
}

/// Estimate complexity of a structure
fn structure_complexity(s: &TermStructure) -> usize {
    match s {
        TermStructure::Gen { .. } | TermStructure::Hole(_) | TermStructure::Reject(_) => 1,
        TermStructure::Compose(parts) => 1 + parts.iter().map(structure_complexity).sum::<usize>(),
        TermStructure::App { args, .. } => 1 + args.iter().map(structure_complexity).sum::<usize>(),
        TermStructure::Doctrine { inner, .. } => 1 + structure_complexity(inner),
        TermStructure::Other(_) => 1,
    }
}

/// Format substitutions for user display
fn format_substitutions(subst: &HashMap<String, TermStructure>) -> String {
    if subst.is_empty() {
        return "Exact match".to_string();
    }
    
    let mut pairs: Vec<_> = subst.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);
    
    pairs.iter()
        .map(|(k, v)| format!("?{} := {}", k, format_structure(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format a TermStructure for display
fn format_structure(s: &TermStructure) -> String {
    match s {
        TermStructure::Gen { index, arity } => format!("gen({}){}", index, arity),
        TermStructure::Compose(parts) => {
            let inner: Vec<_> = parts.iter().map(format_structure).collect();
            format!("({})", inner.join(" ∘ "))
        }
        TermStructure::App { op, args } => {
            let inner: Vec<_> = args.iter().map(format_structure).collect();
            format!("{}({})", op, inner.join(", "))
        }
        TermStructure::Hole(name) => format!("?{}", name),
        TermStructure::Doctrine { scope, inner } => format!("[{}]{}", scope, format_structure(inner)),
        TermStructure::Reject(code) => format!("REJECT({})", code),
        TermStructure::Other(s) => s.clone(),
    }
}

/// Convert a LoogleResult with applicability check into a Proposal
pub fn to_proposal(
    lemma: LoogleResult,
    applicability: ApplicabilityResult,
    anchor: String,
) -> Proposal<LemmaPayload> {
    // Content-addressed proposal ID for determinism
    let id = compute_proposal_id(&anchor, &lemma.name, applicability.confidence);
    
    Proposal {
        id,
        anchor,
        kind: ProposalKind::Lemma,
        payload: LemmaPayload {
            name: lemma.name.clone(),
            doc: lemma.doc.clone(),
            unification_preview: applicability.unification_preview.clone(),
        },
        evidence: EvidenceSummary {
            rationale: applicability.pedagogical_rationale,
            trace_nodes: vec![], // TODO: Add provenance trace
        },
        status: ProposalStatus::Advisory,  // Always advisory until kernel-checked
        reconstruction: Some(ReconstructionPlan {
            engine: "manual".to_string(),
            steps: vec![
                format!("apply_lemma {}", lemma.name),
                "verify_result".to_string(),
            ],
        }),
        score: applicability.confidence,
        truncated: false,
    }
}

/// Compute deterministic proposal ID via SHA256 content addressing
/// Format: sha256("loogle:v1" || anchor || lemma_id || confidence)
fn compute_proposal_id(anchor: &str, lemma_id: &str, confidence: f32) -> String {
    use sha2::{Sha256, Digest};
    
    let mut hasher = Sha256::new();
    hasher.update(b"loogle:v1\x00");
    hasher.update(anchor.as_bytes());
    hasher.update(b"\x00");
    hasher.update(lemma_id.as_bytes());
    hasher.update(b"\x00");
    // Quantize confidence to avoid float repr issues
    hasher.update(format!("{:.2}", confidence).as_bytes());
    
    let result = hasher.finalize();
    format!("loogle_{}", hex::encode(&result[..16])) // Use first 16 bytes for compactness
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LemmaPayload {
    pub name: String,
    pub doc: String,
    pub unification_preview: Option<String>,
}
