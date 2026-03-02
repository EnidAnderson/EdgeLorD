//! HintEngine: Learned tactic ranker for EdgeLorD.
//!
//! **Architecture contract:**
//! - Lives entirely in EdgeLorD (never in comrade_lisp / never crosses Stonewall)
//! - Produces only *suggestions* — the kernel still checks every suggested move
//! - INV D-HINT: output is sorted (score DESC, tactic_id ASC), deterministic for same input
//! - INV T-HINT: HintEngine cannot certify; its results feed `TacticRegistry::compute_all`
//!   results augmented with a score, not bypass it
//!
//! ## Gradient descent relationship (Phase B bridge)
//! The `TacticLog` records `(proof_state_fp, tactic_id, applied)` triples.  These are
//! exactly the `(state, action, success)` triples that form the supervised dataset for
//! a policy learned by gradient descent in Para(Trace2Cat).  When Phase B adds an
//! `optimization_doctrine` the same log can feed an offline trainer that writes a scored
//! model back to `HintModel::load_from_file`.

use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

// ============================================================================
// Public types
// ============================================================================

/// A single ranked suggestion from the HintEngine.
#[derive(Debug, Clone)]
pub struct HintResult {
    /// Stable tactic identifier (maps to `TacticAction::action_id` prefix convention).
    pub tactic_id: String,
    /// Score in [0.0, 1.0]; higher = more likely to be useful.
    pub score: f32,
    /// Human-readable rationale (shown in completion/hover).
    pub rationale: String,
}

/// A record written when a tactic action is *proposed* or *applied*.
///
/// The CSV-formatted log feeds the offline trainer:
/// `epoch_ms,proof_state_fp,tactic_id,applied`
#[derive(Debug, Clone)]
pub struct TacticLogEntry {
    pub epoch_ms: u64,
    /// Hex-encoded SHA-256 of the serialised proof state (or empty if unavailable).
    pub proof_state_fp: String,
    pub tactic_id: String,
    /// `true` = user accepted this action; `false` = it was only suggested.
    pub applied: bool,
}

// ============================================================================
// TacticLog: rolling append-only CSV
// ============================================================================

/// Rolling append-only log of tactic interactions.
///
/// Thread-safe; each call is O(1) I/O (no reads).
/// INV D-LOG: entries are appended in chronological order; no post-sorting needed
///            (training pipeline sorts by proof_state_fp offline).
pub struct TacticLog {
    /// Path to the log file.  `None` = disabled (e.g. tests, no workspace root).
    path: Option<PathBuf>,
    /// Mutex guards the file handle for concurrent writes.
    mutex: Mutex<()>,
}

impl TacticLog {
    /// Create a log at `<workspace_root>/.edgelord/tactic_log.csv`.
    /// Creates parent directories if needed; silently disables on I/O error.
    pub fn new(workspace_root: Option<&Path>) -> Self {
        let path = workspace_root.map(|root| {
            let dir = root.join(".edgelord");
            let _ = fs::create_dir_all(&dir);
            dir.join("tactic_log.csv")
        });

        // Write header if file is new/empty.
        if let Some(ref p) = path {
            if !p.exists() {
                let _ = fs::write(p, "epoch_ms,proof_state_fp,tactic_id,applied\n");
            }
        }

        Self {
            path,
            mutex: Mutex::new(()),
        }
    }

    /// Append one entry.  Silently ignores errors to stay non-blocking.
    pub fn append(&self, entry: &TacticLogEntry) {
        let Some(ref path) = self.path else { return };
        let _guard = self.mutex.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(
                f,
                "{},{},{},{}",
                entry.epoch_ms,
                entry.proof_state_fp,
                // Escape commas inside IDs defensively
                entry.tactic_id.replace(',', "__"),
                if entry.applied { "1" } else { "0" },
            );
        }
    }

    /// Returns true if logging is active (workspace root was provided).
    pub fn is_active(&self) -> bool {
        self.path.is_some()
    }
}

// ============================================================================
// HintModel: pluggable scoring backend
// ============================================================================

/// Internal scoring model.  Initial implementation: frequency-based ranker.
/// Replace `score_inner` with a neural/GNN call once Phase B offline trainer
/// produces a model file.
struct HintModel {
    /// `tactic_id → (proposals, accepts)` aggregated from TacticLog at load time.
    ///
    /// Using BTreeMap for deterministic iteration.
    frequency: BTreeMap<String, (u64, u64)>,
}

impl HintModel {
    fn empty() -> Self {
        Self {
            frequency: BTreeMap::new(),
        }
    }

    /// Load frequency counts from the tactic_log CSV.
    /// Silently returns empty model on any error.
    pub fn load_from_log(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::empty();
        };

        let mut frequency: BTreeMap<String, (u64, u64)> = BTreeMap::new();

        for line in text.lines().skip(1) {
            // epoch_ms, proof_state_fp, tactic_id, applied
            let mut parts = line.splitn(4, ',');
            let _epoch = parts.next();
            let _fp = parts.next();
            let Some(tactic_id) = parts.next() else { continue };
            let applied = parts.next().map(|s| s.trim() == "1").unwrap_or(false);

            let entry = frequency.entry(tactic_id.to_string()).or_insert((0, 0));
            entry.0 += 1; // proposals
            if applied {
                entry.1 += 1; // accepts
            }
        }

        Self { frequency }
    }

    /// Score a tactic_id against this model.
    ///
    /// Returns a value in [0.0, 1.0].
    /// Formula: Wilson lower-bound score (Laplace-smoothed accept rate).
    fn score(&self, tactic_id: &str) -> f32 {
        let (proposals, accepts) = self
            .frequency
            .get(tactic_id)
            .copied()
            .unwrap_or((0, 0));
        // Laplace smoothing: (accepts + 1) / (proposals + 2)
        let p = (accepts + 1) as f32 / (proposals + 2) as f32;
        // Clamp to avoid NaN
        p.clamp(0.0, 1.0)
    }
}

// ============================================================================
// HintEngine: public service
// ============================================================================

/// Service for ranking tactic actions by learned utility.
///
/// Thread-safe Arc wrapper; held on the Backend struct as `Arc<HintEngine>`.
pub struct HintEngine {
    log: Arc<TacticLog>,
    /// Model is refreshed lazily; wrapped in Mutex for interior mutability.
    model: Mutex<HintModel>,
    log_path: Option<PathBuf>,
}

impl HintEngine {
    /// Create a HintEngine. `workspace_root` may be None (no logging/learning).
    pub fn new(workspace_root: Option<&Path>) -> Arc<Self> {
        let log_path = workspace_root.map(|r| r.join(".edgelord").join("tactic_log.csv"));
        let model = match &log_path {
            Some(p) if p.exists() => HintModel::load_from_log(p),
            _ => HintModel::empty(),
        };

        Arc::new(Self {
            log: Arc::new(TacticLog::new(workspace_root)),
            model: Mutex::new(model),
            log_path,
        })
    }

    // -----------------------------------------------------------------------
    // Query
    // -----------------------------------------------------------------------

    /// Return ranked hints for the given tactic candidates.
    ///
    /// `proof_state_bytes` is any stable serialisation of the current proof state
    /// (used as the key for future per-state conditioning; currently ignored by
    /// the frequency model but logged for the offline trainer).
    ///
    /// **INV D-HINT**: output sorted by (score DESC, tactic_id ASC).
    pub fn query(
        &self,
        proof_state_bytes: &[u8],
        tactic_ids: &[String],
    ) -> Vec<HintResult> {
        let state_fp = proof_state_fingerprint(proof_state_bytes);
        let model = self.model.lock().unwrap_or_else(|e| e.into_inner());

        let mut results: Vec<HintResult> = tactic_ids
            .iter()
            .map(|id| {
                let score = model.score(id);
                HintResult {
                    tactic_id: id.clone(),
                    score,
                    rationale: format!(
                        "HintEngine frequency score {:.2} (state {})",
                        score,
                        &state_fp[..8],
                    ),
                }
            })
            .collect();

        // INV D-HINT: deterministic sort
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.tactic_id.cmp(&b.tactic_id))
        });

        results
    }

    // -----------------------------------------------------------------------
    // Logging
    // -----------------------------------------------------------------------

    /// Record that a batch of tactic_ids was *proposed* for a proof state.
    pub fn record_proposed(&self, proof_state_bytes: &[u8], tactic_ids: &[String]) {
        if !self.log.is_active() {
            return;
        }
        let fp = proof_state_fingerprint(proof_state_bytes);
        let epoch_ms = unix_ms();
        for id in tactic_ids {
            self.log.append(&TacticLogEntry {
                epoch_ms,
                proof_state_fp: fp.clone(),
                tactic_id: id.clone(),
                applied: false,
            });
        }
    }

    /// Record that a specific tactic_id was *applied* (user accepted).
    pub fn record_applied(&self, proof_state_bytes: &[u8], tactic_id: &str) {
        if !self.log.is_active() {
            return;
        }
        self.log.append(&TacticLogEntry {
            epoch_ms: unix_ms(),
            proof_state_fp: proof_state_fingerprint(proof_state_bytes),
            tactic_id: tactic_id.to_string(),
            applied: true,
        });
    }

    /// Reload the scoring model from the log (cheap; call periodically).
    pub fn refresh_model(&self) {
        let new_model = match &self.log_path {
            Some(p) if p.exists() => HintModel::load_from_log(p),
            _ => return,
        };
        let mut model = self.model.lock().unwrap_or_else(|e| e.into_inner());
        *model = new_model;
    }

    /// Expose the log for injection from `execute_command` handler.
    pub fn log(&self) -> &Arc<TacticLog> {
        &self.log
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Stable 64-char hex fingerprint of arbitrary bytes.
fn proof_state_fingerprint(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"HINT_STATE_V1");
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn hint_engine_deterministic_output() {
        let tmp = TempDir::new().unwrap();
        let engine = HintEngine::new(Some(tmp.path()));

        let state = b"proof_state_bytes_stub";
        let ids = vec![
            "std.rewrite".to_string(),
            "std.goal_directed".to_string(),
            "std.quickfix".to_string(),
        ];

        let r1 = engine.query(state, &ids);
        let r2 = engine.query(state, &ids);

        // INV D-HINT: identical inputs → identical outputs
        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.tactic_id, b.tactic_id);
            assert_eq!((a.score * 1000.0) as i32, (b.score * 1000.0) as i32);
        }
    }

    #[test]
    fn hint_engine_sorted_output() {
        let engine = HintEngine::new(None);
        let state = b"";
        let ids: Vec<String> = vec!["z_tactic".into(), "a_tactic".into(), "m_tactic".into()];
        let results = engine.query(state, &ids);
        // With empty model all scores equal; should be sorted by tactic_id ASC
        let names: Vec<&str> = results.iter().map(|r| r.tactic_id.as_str()).collect();
        assert_eq!(names, vec!["a_tactic", "m_tactic", "z_tactic"]);
    }

    #[test]
    fn tactic_log_append_and_reload() {
        let tmp = TempDir::new().unwrap();
        let engine = HintEngine::new(Some(tmp.path()));

        let state = b"state42";
        // Simulate 3 proposals, 2 applied
        engine.record_proposed(state, &["std.rewrite".into(), "std.quickfix".into()]);
        engine.record_applied(state, "std.rewrite");
        engine.record_proposed(state, &["std.rewrite".into()]);
        engine.record_applied(state, "std.rewrite");

        engine.refresh_model();

        let results = engine.query(state, &["std.rewrite".into(), "std.quickfix".into()]);
        // std.rewrite was applied twice out of 3 proposals → score > std.quickfix
        let rewrite_score = results.iter().find(|r| r.tactic_id == "std.rewrite").unwrap().score;
        let qf_score = results.iter().find(|r| r.tactic_id == "std.quickfix").unwrap().score;
        assert!(rewrite_score > qf_score, "rewrite ({}) should score higher than quickfix ({})", rewrite_score, qf_score);
    }
}
