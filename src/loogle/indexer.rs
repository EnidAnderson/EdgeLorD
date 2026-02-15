use super::LoogleIndex;
use comrade_lisp::core::{CoreBundleV0, CompiledRule};

/// Fingerprint format version - increment when changing fingerprint structure
/// to invalidate stale indexes and prevent version drift.
pub const LOOGLE_FP_VERSION: u32 = 1;

/// Maximum recursion depth for fingerprinting to prevent runaway on cyclic structures
const MAX_FP_DEPTH: usize = 32;

/// Maximum number of nodes to fingerprint before truncation
const MAX_FP_NODES: usize = 256;

/// Truncation marker for fingerprints that exceeded bounds
const TRUNCATION_MARKER: &str = "…";

/// Extracts and indexes lemmas from a workspace bundle
pub struct WorkspaceIndexer {
    index: LoogleIndex,
}

impl WorkspaceIndexer {
    pub fn new() -> tantivy::Result<Self> {
        Ok(Self {
            index: LoogleIndex::new_in_memory()?,
        })
    }

    /// Re-index the entire workspace from a new bundle
    pub fn reindex(&self, bundle: &CoreBundleV0) -> tantivy::Result<()> {
        // Clear existing index
        self.index.clear()?;
        
        // Extract lemmas from compiled rules
        for rule in &bundle.rules {
            if is_lemma(&rule) {
                let name = extract_lemma_name(&rule);
                let lhs_fp = compute_fingerprint(&rule.lhs);
                let rhs_fp = compute_fingerprint(&rule.rhs);
                let doc = extract_doc_string(&rule);
                let provenance = rule.meta.provenance.as_deref();
                
                self.index.index_lemma(
                    &name,
                    &lhs_fp,
                    &rhs_fp,
                    &doc,
                    provenance,
                )?;
            }
        }
        
        Ok(())
    }

    /// Get the underlying index for search operations
    pub fn index(&self) -> &LoogleIndex {
        &self.index
    }
}

/// Check if a rule is marked as a lemma
fn is_lemma(rule: &CompiledRule) -> bool {
    rule.meta.classes.contains("lemma") || rule.meta.classes.contains("simp")
}

/// Extract a human-readable name from the rule metadata or synthesize one
fn extract_lemma_name(rule: &CompiledRule) -> String {
    // Try to extract name from provenance
    if let Some(prov) = &rule.meta.provenance {
        return prov.clone();
    }
    
    // Otherwise synthesize from LHS structure
    format!("rule_{:x}", compute_hash(&rule.lhs))
}

/// Compute a structural fingerprint for search indexing
/// 
/// This produces a canonical, deterministic string representation
/// of the term structure that can be used for structural search.
/// The format captures term shape while abstracting over specific IDs.
/// 
/// Format: `v{VERSION}:{fingerprint}`
/// 
/// **Invariant G6**: No Debug-derived strings used in fingerprints.
/// All formatting uses stable, versioned accessors.
pub fn compute_fingerprint(term: &tcb_core::ast::MorphismTerm) -> String {
    let mut ctx = FingerprintContext::new();
    let inner = compute_fingerprint_bounded(term, &mut ctx);
    format!("v{}:{}", LOOGLE_FP_VERSION, inner)
}

/// Fingerprinting context for tracking bounds
struct FingerprintContext {
    depth: usize,
    nodes: usize,
    truncated: bool,
}

impl FingerprintContext {
    fn new() -> Self {
        Self {
            depth: 0,
            nodes: 0,
            truncated: false,
        }
    }
    
    /// Check if we've exceeded bounds
    fn exceeded(&self) -> bool {
        self.depth > MAX_FP_DEPTH || self.nodes > MAX_FP_NODES
    }
    
    /// Enter a child node, returning true if we should process it
    fn enter(&mut self) -> bool {
        self.depth += 1;
        self.nodes += 1;
        !self.exceeded()
    }
    
    /// Exit a child node
    fn exit(&mut self) {
        self.depth -= 1;
    }
    
    /// Mark as truncated and return truncation marker
    fn truncate(&mut self) -> String {
        self.truncated = true;
        TRUNCATION_MARKER.to_string()
    }
}

/// Bounded fingerprinting with depth/node tracking
fn compute_fingerprint_bounded(
    term: &tcb_core::ast::MorphismTerm,
    ctx: &mut FingerprintContext,
) -> String {
    use tcb_core::ast::MorphismTerm;
    
    if !ctx.enter() {
        return ctx.truncate();
    }
    
    let result = match term {
        MorphismTerm::Generator { id, inputs, outputs } => {
            // Include generator index and arity signature
            // Stable: GeneratorId::index() returns u32
            format!("gen:{}:[{}→{}]", 
                id.index(), 
                inputs.len(), 
                outputs.len())
        }
        MorphismTerm::Compose { components, inputs, outputs, .. } => {
            // Recursively fingerprint components
            let parts: Vec<String> = components.iter()
                .map(|c| compute_fingerprint_bounded(c, ctx))
                .collect();
            format!("comp:[{}]:[{}→{}]", 
                parts.join(";"), 
                inputs.len(), 
                outputs.len())
        }
        MorphismTerm::App { op, args, inputs, outputs } => {
            // Fingerprint constructor application using stable index
            // Stable: ConstructorId.index is a u32 field
            let args_str = args.iter()
                .map(format_term_arg)
                .collect::<Vec<_>>()
                .join(",");
            format!("app:{}({}):[{}→{}]", 
                op.index, 
                args_str, 
                inputs.len(), 
                outputs.len())
        }
        MorphismTerm::InDoctrine { doctrine, term } => {
            // FIXED: Use stable as_u32() instead of Debug formatting
            // Stable: DoctrineKey::as_u32() returns the raw u32 index
            let inner = compute_fingerprint_bounded(term, ctx);
            format!("doc:{}:{}", doctrine.as_u32(), inner)
        }
        MorphismTerm::Hole(id) => {
            // Holes are pattern variables - use the numeric ID
            // Stable: HoleId implements Display with stable format
            format!("?{}", id)
        }
        MorphismTerm::Reject { code, .. } => {
            // Rejected terms - include error code (user-provided string, stable)
            format!("!{}", code)
        }    };
    
    ctx.exit();
    result
}

/// Format a term argument for fingerprinting
fn format_term_arg(arg: &tcb_core::ast::constructor_registry::TermArg) -> String {
    use tcb_core::ast::constructor_registry::TermArg;
    match arg {
        TermArg::Object(oid) => format!("o:{}", oid.index()),
        TermArg::Morphism(mid) => format!("m:{}", mid.index()),
    }
}

/// Compute stable hash for synthesized names
fn compute_hash(term: &tcb_core::ast::MorphismTerm) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // Hash the structural fingerprint for determinism
    let fp = compute_fingerprint(term);
    let mut hasher = DefaultHasher::new();
    fp.hash(&mut hasher);
    hasher.finish()
}

/// Extract documentation string from rule metadata
fn extract_doc_string(rule: &CompiledRule) -> String {
    // For now, return a placeholder
    // TODO: Extract from metadata s-expr
    format!("Rewrite rule with provenance: {}", 
        rule.meta.provenance.as_deref().unwrap_or("unknown"))
}
