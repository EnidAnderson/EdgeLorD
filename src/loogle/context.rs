/// Goal context module for cursor-position-aware search
/// 
/// Provides relevance scoring for lemma suggestions based on the
/// current proof context (goal fingerprint, surrounding bindings).

use super::LoogleResult;

/// Context information about the current goal being proved
#[derive(Debug, Clone)]
pub struct GoalContext {
    /// Structural fingerprint of the current goal
    pub goal_fingerprint: String,
    
    /// Names of locally bound variables in scope
    pub surrounding_bindings: Vec<String>,
    
    /// Optional cursor position (line, column)
    pub cursor_position: Option<(usize, usize)>,
}

impl GoalContext {
    /// Create a new goal context
    pub fn new(goal_fingerprint: String) -> Self {
        Self {
            goal_fingerprint,
            surrounding_bindings: vec![],
            cursor_position: None,
        }
    }
    
    /// Add surrounding bindings
    pub fn with_bindings(mut self, bindings: Vec<String>) -> Self {
        self.surrounding_bindings = bindings;
        self
    }
    
    /// Add cursor position
    pub fn with_cursor(mut self, line: usize, col: usize) -> Self {
        self.cursor_position = Some((line, col));
        self
    }
    
    /// Compute relevance score for a lemma in this context
    /// 
    /// Returns a score between 0.0 and 1.0 indicating how relevant
    /// the lemma is to the current goal context.
    pub fn relevance_score(&self, lemma: &LoogleResult) -> f32 {
        let base = 0.5;
        
        // Boost for structural similarity (fingerprint substring match)
        let structural_boost = if fingerprint_similarity(&self.goal_fingerprint, &lemma.rationale) > 0.5 {
            0.3
        } else if fingerprint_similarity(&self.goal_fingerprint, &lemma.rationale) > 0.2 {
            0.15
        } else {
            0.0
        };
        
        // Boost for matching binding names
        let binding_boost = self.count_matching_bindings(lemma) as f32 * 0.05;
        
        (base + structural_boost + binding_boost).min(1.0)
    }
    
    /// Count how many local bindings are mentioned in the lemma
    fn count_matching_bindings(&self, lemma: &LoogleResult) -> usize {
        self.surrounding_bindings.iter()
            .filter(|b| lemma.name.contains(b.as_str()) || lemma.doc.contains(b.as_str()))
            .count()
    }
    
    /// Filter and sort lemma results by relevance to this context
    pub fn rank_results(&self, results: Vec<LoogleResult>) -> Vec<(LoogleResult, f32)> {
        let mut scored: Vec<_> = results.into_iter()
            .map(|r| {
                let score = self.relevance_score(&r);
                (r, score)
            })
            .collect();
        
        // Sort by score descending, then by name for stability
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.name.cmp(&b.0.name))
        });
        
        scored
    }
}

/// Compute approximate similarity between two fingerprints
/// 
/// Uses token overlap as a simple heuristic for structural similarity.
fn fingerprint_similarity(fp1: &str, fp2: &str) -> f32 {
    let tokens1: std::collections::HashSet<_> = fp1.split(&[':', '[', ']', '(', ')', ',', ';', '→'][..])
        .filter(|s| !s.is_empty())
        .collect();
    
    let tokens2: std::collections::HashSet<_> = fp2.split(&[':', '[', ']', '(', ')', ',', ';', '→'][..])
        .filter(|s| !s.is_empty())
        .collect();
    
    if tokens1.is_empty() || tokens2.is_empty() {
        return 0.0;
    }
    
    let intersection = tokens1.intersection(&tokens2).count();
    let union = tokens1.union(&tokens2).count();
    
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_goal_context_basic() {
        let ctx = GoalContext::new("gen:0:[1→1]".to_string());
        assert_eq!(ctx.goal_fingerprint, "gen:0:[1→1]");
        assert!(ctx.surrounding_bindings.is_empty());
    }
    
    #[test]
    fn test_fingerprint_similarity_identical() {
        let sim = fingerprint_similarity("gen:0:[1→1]", "gen:0:[1→1]");
        assert!((sim - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_fingerprint_similarity_different() {
        let sim = fingerprint_similarity("gen:0:[1→1]", "app:tensor(x,y):[2→2]");
        assert!(sim < 0.5);
    }
}
