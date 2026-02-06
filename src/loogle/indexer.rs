use super::LoogleIndex;
use new_surface_syntax::core::{CoreBundleV0, CompiledRule};

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
fn compute_fingerprint(term: &tcb_core::ast::MorphismTerm) -> String {
    // For now, just return a placeholder until we have proper access to term structure
    format!("{:?}", term)
}

/// Simple hash for synthesized names
fn compute_hash(term: &tcb_core::ast::MorphismTerm) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    format!("{:?}", term).hash(&mut hasher);
    hasher.finish()
}

/// Extract documentation string from rule metadata
fn extract_doc_string(rule: &CompiledRule) -> String {
    // For now, return a placeholder
    // TODO: Extract from metadata s-expr
    format!("Rewrite rule with provenance: {}", 
        rule.meta.provenance.as_deref().unwrap_or("unknown"))
}
