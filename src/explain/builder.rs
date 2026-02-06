use std::collections::{BTreeMap, VecDeque, HashSet};
use source_span::Span;
use crate::explain::view::{ExplanationNode, ExplanationKind, ExplanationView, ExplainLimits};

/// Internal record for a node in the arena.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub id: String,
    pub kind: ExplanationKind,
    pub label: String,
    pub jump_target: Option<Span>,
    pub metadata: BTreeMap<String, String>,
    pub children_indices: Vec<usize>,
    pub depth: usize,
}

/// Arena-based builder to avoid borrow-check pain and ensure deterministic materialization.
pub struct ExplainBuilder {
    limits: ExplainLimits,
    arena: Vec<NodeRecord>,
    queue: VecDeque<usize>, // Arena indices
    visited: HashSet<String>,
    
    total_nodes: usize,
    truncated: bool,
    truncation_reason: Option<String>,
    
    root_idx: Option<usize>,
}

impl ExplainBuilder {
    pub fn new(limits: ExplainLimits) -> Self {
        Self {
            limits,
            arena: Vec::new(),
            queue: VecDeque::new(),
            visited: HashSet::new(),
            total_nodes: 0,
            truncated: false,
            truncation_reason: None,
            root_idx: None,
        }
    }

    pub fn set_root(&mut self, id: String, kind: ExplanationKind, label: String, span: Option<Span>) -> usize {
        let record = NodeRecord {
            id: id.clone(),
            kind,
            label,
            jump_target: span,
            metadata: BTreeMap::new(),
            children_indices: Vec::new(),
            depth: 0,
        };
        let idx = self.arena.len();
        self.arena.push(record);
        self.root_idx = Some(idx);
        self.queue.push_back(idx);
        self.visited.insert(id);
        self.total_nodes = 1;
        idx
    }

    pub fn add_metadata(&mut self, node_idx: usize, key: String, value: String) {
        if let Some(node) = self.arena.get_mut(node_idx) {
            node.metadata.insert(key, value);
        }
    }

    /// Add a child to a parent node.
    /// Returns Some(child_idx) if added, None if blocked by limits or already visited.
    pub fn add_child(&mut self, parent_idx: usize, id: String, kind: ExplanationKind, label: String, span: Option<Span>) -> Option<usize> {
        if self.visited.contains(&id) {
            return None;
        }

        if self.total_nodes >= self.limits.max_nodes {
            self.set_truncated("limit");
            return None;
        }

        let parent_depth = self.arena[parent_idx].depth;
        if parent_depth + 1 > self.limits.max_depth {
            self.set_truncated("depth");
            return None;
        }

        if self.arena[parent_idx].children_indices.len() >= self.limits.max_children_per_node {
            self.set_truncated("limit"); // Child limit hit
            return None;
        }

        let child_record = NodeRecord {
            id: id.clone(),
            kind,
            label,
            jump_target: span,
            metadata: BTreeMap::new(),
            children_indices: Vec::new(),
            depth: parent_depth + 1,
        };

        let child_idx = self.arena.len();
        self.arena.push(child_record);
        self.arena[parent_idx].children_indices.push(child_idx);
        self.queue.push_back(child_idx);
        self.visited.insert(id);
        self.total_nodes += 1;
        
        Some(child_idx)
    }

    fn set_truncated(&mut self, reason: &str) {
        self.truncated = true;
        if self.truncation_reason.is_none() {
            self.truncation_reason = Some(reason.to_string());
        }
    }

    pub fn next_idx(&mut self) -> Option<usize> {
        self.queue.pop_front()
    }

    pub fn get_node(&self, idx: usize) -> &NodeRecord {
        &self.arena[idx]
    }

    /// Build the final ExplanationView.
    /// Performs deterministic sorting of children before materializing.
    pub fn build(self) -> ExplanationView {
        let root_idx = self.root_idx.expect("Root must be set before building");
        
        // Materialize root
        let root = self.materialize_node(root_idx);

        ExplanationView {
            root,
            total_nodes: self.total_nodes,
            truncated: self.truncated,
            truncation_reason: self.truncation_reason,
            traversal: "bfs".to_string(),
        }
    }

    fn materialize_node(&self, idx: usize) -> ExplanationNode {
        let node_ref = &self.arena[idx];
        
        // Clone and sort children indices deterministically
        let mut children_indices = node_ref.children_indices.clone();
        children_indices.sort_by(|&a, &b| {
            let na = &self.arena[a];
            let nb = &self.arena[b];
            
            (na.kind as u8).cmp(&(nb.kind as u8))
                .then(na.jump_target.map(|s| s.start).unwrap_or(usize::MAX).cmp(&nb.jump_target.map(|s| s.start).unwrap_or(usize::MAX)))
                .then(na.jump_target.map(|s| s.end).unwrap_or(usize::MAX).cmp(&nb.jump_target.map(|s| s.end).unwrap_or(usize::MAX)))
                .then(na.id.cmp(&nb.id))
        });

        let children = children_indices.into_iter()
            .map(|c_idx| self.materialize_node(c_idx))
            .collect();

        ExplanationNode {
            id: node_ref.id.clone(),
            kind: node_ref.kind,
            label: node_ref.label.clone(),
            jump_target: node_ref.jump_target,
            children,
            metadata: node_ref.metadata.clone(),
        }
    }
}

pub fn truncate_label(label: String, max_chars: usize) -> String {
    if label.len() <= max_chars {
        label
    } else {
        let truncate_at = max_chars.saturating_sub(1);
        format!("{}…", &label[..truncate_at])
    }
}
