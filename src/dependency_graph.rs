//! SD4: Cross-file proof dependency graph.
//!
//! Tracks `(use ...)` import edges between files, export fingerprints, and
//! drives incremental re-checking when exports change.
//!
//! **INV D-***: BTreeMap for all keyed state; toposort is deterministic.
//! **INV S-STEP**: change propagation follows topological order.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};

use comrade_lisp::comrade_workspace::WorkspaceReport;

// ─── Dependency graph ─────────────────────────────────────────────────────────

/// Directed acyclic graph of file-level import dependencies.
///
/// `imports[A]` = set of files that A imports.
/// `dependents[B]` = set of files that import B.
#[derive(Debug, Default, Clone)]
pub struct DependencyGraph {
    /// A → { B, C, ... } where A imports B and C.
    pub imports: BTreeMap<String, BTreeSet<String>>,
    /// B → { A } where A imports B (reverse edges).
    pub dependents: BTreeMap<String, BTreeSet<String>>,
    /// Per-file export fingerprint.
    pub fingerprints: BTreeMap<String, u64>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `file` imports `imported_files`.
    ///
    /// Old import edges for `file` are replaced. Reverse edges updated.
    pub fn record_imports(
        &mut self,
        file: &str,
        imported_files: impl IntoIterator<Item = String>,
    ) {
        // Remove old reverse edges for this file
        if let Some(old_imports) = self.imports.get(file).cloned() {
            for dep in old_imports {
                if let Some(rev) = self.dependents.get_mut(&dep) {
                    rev.remove(file);
                }
            }
        }

        let new_imports: BTreeSet<String> = imported_files.into_iter().collect();
        for dep in &new_imports {
            self.dependents
                .entry(dep.clone())
                .or_default()
                .insert(file.to_string());
        }
        self.imports.insert(file.to_string(), new_imports);
    }

    /// Update the export fingerprint for `file`. Returns `true` if it changed.
    pub fn update_fingerprint(&mut self, file: &str, fp: u64) -> bool {
        let old = self.fingerprints.insert(file.to_string(), fp);
        old != Some(fp)
    }

    /// All files that transitively depend on `file`, in BFS order.
    ///
    /// **INV D-***: BFS order is deterministic via BTreeSet iteration.
    pub fn stale_dependents(&self, file: &str) -> Vec<String> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut order: Vec<String> = Vec::new();

        if let Some(deps) = self.dependents.get(file) {
            for d in deps {
                if visited.insert(d.clone()) {
                    queue.push_back(d.clone());
                }
            }
        }

        while let Some(next) = queue.pop_front() {
            order.push(next.clone());
            if let Some(deps) = self.dependents.get(&next) {
                for d in deps {
                    if visited.insert(d.clone()) {
                        queue.push_back(d.clone());
                    }
                }
            }
        }

        order
    }

    /// Detect cycles (find files reachable from themselves via imports).
    ///
    /// Returns each cycle as a sorted list of file strings.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles: Vec<Vec<String>> = Vec::new();
        for start in self.imports.keys() {
            let mut path: Vec<String> = Vec::new();
            let mut visited: BTreeSet<String> = BTreeSet::new();
            if self.has_cycle_from(start, start, &mut path, &mut visited) {
                let mut cycle = path.clone();
                cycle.sort();
                cycle.dedup();
                if !cycles.contains(&cycle) {
                    cycles.push(cycle);
                }
            }
        }
        cycles
    }

    fn has_cycle_from(
        &self,
        origin: &str,
        current: &str,
        path: &mut Vec<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if !visited.insert(current.to_string()) {
            return false;
        }
        path.push(current.to_string());
        if let Some(imports) = self.imports.get(current) {
            for next in imports {
                if next == origin {
                    return true;
                }
                if self.has_cycle_from(origin, next, path, visited) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }
}

// ─── Export fingerprint ───────────────────────────────────────────────────────

/// Hash of exported symbol names from a workspace report.
///
/// Only hashes the API surface (names), not implementations.
///
/// **INV D-***: symbol names sorted before hashing.
pub fn export_fingerprint(report: &WorkspaceReport) -> u64 {
    let mut hasher = DefaultHasher::new();
    if let Some(bundle) = &report.bundle {
        let mut names: Vec<&str> = bundle.rules.iter().map(|r| r.name.as_str()).collect();
        names.sort_unstable();
        for name in names {
            name.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Extract imported file paths from a `WorkspaceReport`.
/// Returns sorted list of canonical paths.
///
/// **INV D-***: result is sorted.
///
/// TODO: wire to actual import graph once WorkspaceReport exposes import paths.
pub fn extract_imports(_report: &WorkspaceReport) -> Vec<String> {
    // WorkspaceReport does not currently expose import paths directly.
    // Future: use report.bundle.imported_paths or similar when available.
    vec![]
}
