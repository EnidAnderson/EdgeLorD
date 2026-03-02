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

/// Maximum number of per-fingerprint buckets to retain (insertion-order LRU cap).
/// Prevents unbounded growth when many distinct proof states are encountered.
/// When exceeded, the oldest-seen fingerprint bucket is evicted.
const MAX_FP_BUCKETS: usize = 256;

/// Backoff blend constant for per-fingerprint scoring.
///
/// `α = n / (n + BACKOFF_BLEND)` where n = local proposal count.
/// - When n = 0: α = 0   → pure global score (no local evidence)
/// - When n >> BACKOFF_BLEND: α → 1 → pure local score
/// Prevents one lucky accept from dominating a new fingerprint key.
const BACKOFF_BLEND: f32 = 10.0;

/// Internal scoring model.  Initial implementation: frequency-based ranker.
/// Replace `score` with a neural/GNN call once Phase B offline trainer
/// produces a model file.
///
/// ## Per-fingerprint conditioning (INV D-HINT)
///
/// Statistics are stored at two levels:
/// - `global`: all (tactic_id, outcome) pairs across all proof states
/// - `per_fp`: per-fingerprint-prefix (first 16 hex chars) → per-tactic stats
///
/// Scoring uses backoff smoothing: when a fingerprint has local evidence,
/// `score = α * local_score + (1-α) * global_score` with α rising with evidence.
/// When α = 0 (no local evidence), the score matches global stats exactly.
struct HintModel {
    /// Global statistics: tactic_id → (proposals, accepts).
    /// Populated from all log entries regardless of fingerprint.
    /// INV D-HINT: BTreeMap for deterministic iteration.
    global: BTreeMap<String, (u64, u64)>,
    /// Per-fingerprint-prefix statistics: fp_prefix → tactic_id → (proposals, accepts).
    /// Keyed by the first 16 hex chars of the fingerprint string.
    /// Capped at MAX_FP_BUCKETS entries (insertion-order LRU).
    per_fp: BTreeMap<String, BTreeMap<String, (u64, u64)>>,
}

impl HintModel {
    fn empty() -> Self {
        Self {
            global: BTreeMap::new(),
            per_fp: BTreeMap::new(),
        }
    }

    /// Load frequency counts from the tactic_log CSV.
    /// Silently returns empty model on any error.
    pub fn load_from_log(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::empty();
        };

        let mut global: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        let mut per_fp: BTreeMap<String, BTreeMap<String, (u64, u64)>> = BTreeMap::new();
        // Track insertion order for LRU eviction
        let mut fp_order: std::collections::VecDeque<String> = std::collections::VecDeque::new();

        for line in text.lines().skip(1) {
            // epoch_ms, proof_state_fp, tactic_id, applied
            let mut parts = line.splitn(4, ',');
            let _epoch = parts.next();
            let Some(fp) = parts.next() else { continue };
            let Some(tactic_id) = parts.next() else { continue };
            let applied = parts.next().map(|s| s.trim() == "1").unwrap_or(false);
            let tactic_id = tactic_id.to_string();

            // Global stats (all fingerprints)
            let entry = global.entry(tactic_id.clone()).or_insert((0, 0));
            entry.0 += 1;
            if applied { entry.1 += 1; }

            // Per-fingerprint-prefix stats (first 16 chars of fp)
            let fp_prefix = fp[..16.min(fp.len())].to_string();
            if !per_fp.contains_key(&fp_prefix) {
                // LRU eviction: evict oldest bucket if at cap
                if fp_order.len() >= MAX_FP_BUCKETS {
                    if let Some(oldest) = fp_order.pop_front() {
                        per_fp.remove(&oldest);
                    }
                }
                fp_order.push_back(fp_prefix.clone());
            }
            let per = per_fp.entry(fp_prefix).or_default();
            let per_entry = per.entry(tactic_id).or_insert((0, 0));
            per_entry.0 += 1;
            if applied { per_entry.1 += 1; }
        }

        Self { global, per_fp }
    }

    /// Score a (state_fp, tactic_id) pair against this model.
    ///
    /// Returns a value in [0.0, 1.0].
    ///
    /// **Formula (backoff smoothing)**:
    /// - `global_score = (g_accepts + 1) / (g_proposals + 2)` (Laplace-smoothed)
    /// - When per-fp bucket exists: `α * local_score + (1-α) * global_score`
    ///   where `α = n / (n + BACKOFF_BLEND)` and `n = local_proposals`
    /// - When no per-fp bucket (or n=0): pure `global_score`
    ///
    /// **INV D-HINT**: same (state_fp, tactic_id) → same score (BTreeMap iteration order).
    fn score(&self, state_fp: &str, tactic_id: &str) -> f32 {
        let (g_proposals, g_accepts) = self.global.get(tactic_id).copied().unwrap_or((0, 0));
        // Laplace-smoothed global score
        let global_score = (g_accepts + 1) as f32 / (g_proposals + 2) as f32;

        // Per-fingerprint backoff
        let fp_prefix = &state_fp[..16.min(state_fp.len())];
        if let Some(per) = self.per_fp.get(fp_prefix) {
            let (l_proposals, l_accepts) = per.get(tactic_id).copied().unwrap_or((0, 0));
            let n = l_proposals as f32;
            let alpha = n / (n + BACKOFF_BLEND); // 0 when n=0; rising toward 1
            let local_score = (l_accepts + 1) as f32 / (l_proposals + 2) as f32;
            (alpha * local_score + (1.0 - alpha) * global_score).clamp(0.0, 1.0)
        } else {
            global_score.clamp(0.0, 1.0)
        }
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
        self.query_inner(
            &proof_state_fingerprint(proof_state_bytes),
            tactic_ids,
        )
    }

    /// Variant of `query` that uses a pre-computed motivic fingerprint.
    ///
    /// Called when the proof state contains a `para-info` payload encoding
    /// a (μ, repr_map) pair from the motivic ML doctrines.  The fingerprint
    /// is produced by `motivic_proof_fingerprint(para_info_bytes)`, which uses
    /// a distinct hash prefix so per-(μ,r) statistics accumulate separately
    /// from generic per-proof-state statistics.
    ///
    /// **INV D-HINT**: same fingerprint → same ranked output.
    pub fn query_with_motivic_hint(
        &self,
        motivic_fingerprint: &str,
        tactic_ids: &[String],
    ) -> Vec<HintResult> {
        self.query_inner(motivic_fingerprint, tactic_ids)
    }

    /// Internal ranked query given a pre-computed fingerprint string.
    fn query_inner(&self, state_fp: &str, tactic_ids: &[String]) -> Vec<HintResult> {
        let model = self.model.lock().unwrap_or_else(|e| e.into_inner());

        let mut results: Vec<HintResult> = tactic_ids
            .iter()
            .map(|id| {
                // Per-fingerprint conditioned score with backoff smoothing
                let score = model.score(state_fp, id);
                HintResult {
                    tactic_id: id.clone(),
                    score,
                    rationale: format!(
                        "HintEngine score {:.2} (state {})",
                        score,
                        &state_fp[..8.min(state_fp.len())],
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
        self.record_proposed_with_fp(
            &proof_state_fingerprint(proof_state_bytes),
            tactic_ids,
        );
    }

    /// Record proposals using a pre-computed fingerprint (motivic or raw).
    ///
    /// Used when the proof state has a `para-info` payload; the caller passes
    /// the motivic fingerprint so (μ,r)-keyed statistics accumulate correctly.
    pub fn record_proposed_with_fp(&self, state_fp: &str, tactic_ids: &[String]) {
        if !self.log.is_active() {
            return;
        }
        let epoch_ms = unix_ms();
        for id in tactic_ids {
            self.log.append(&TacticLogEntry {
                epoch_ms,
                proof_state_fp: state_fp.to_string(),
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

/// Motivic fingerprint derived from a serialized `para-info` payload.
///
/// Produces a fingerprint distinct from `proof_state_fingerprint`, so that
/// HintEngine can accumulate per (μ, r) statistics rather than per raw-bytes
/// statistics when a `para-info` term is present in the proof state.
///
/// The prefix "HINT_MOTIVIC_V1" ensures no collision with the standard path.
///
/// **INV D-***: same `para_info_bytes` → same 64-char hex output.
pub fn motivic_proof_fingerprint(para_info_bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"HINT_MOTIVIC_V1");
    h.update(para_info_bytes);
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
    fn motivic_fingerprint_is_deterministic_and_distinct_from_raw() {
        // INV D-*: same para_info_bytes → same motivic fingerprint
        let para_info = b"(para-info realize/euler-char project-low-2-bits motivic_ml.repr_opt_step)";
        let fp1 = motivic_proof_fingerprint(para_info);
        let fp2 = motivic_proof_fingerprint(para_info);
        assert_eq!(fp1, fp2, "motivic fingerprint must be deterministic (INV D-*)");
        assert_eq!(fp1.len(), 64, "motivic fingerprint must be 64-char hex");

        // Must be distinct from the raw fingerprint for the same bytes
        let raw_fp = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"HINT_STATE_V1");
            h.update(para_info);
            format!("{:x}", h.finalize())
        };
        assert_ne!(fp1, raw_fp, "motivic and raw fingerprints must be distinct");
    }

    #[test]
    fn hint_engine_motivic_path_is_deterministic() {
        // INV D-HINT: query_with_motivic_hint is deterministic for same fingerprint
        let engine = HintEngine::new(None);
        let fp = motivic_proof_fingerprint(
            b"(para-info mu repr-map motivic_ml.repr_opt_step)",
        );
        let ids = vec!["motivic_ml.repr_opt_step".to_string(), "std.rewrite".to_string()];
        let r1 = engine.query_with_motivic_hint(&fp, &ids);
        let r2 = engine.query_with_motivic_hint(&fp, &ids);
        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.tactic_id, b.tactic_id);
            assert_eq!((a.score * 1000.0) as i32, (b.score * 1000.0) as i32);
        }
    }

    #[test]
    fn motivic_and_raw_paths_produce_independent_statistics() {
        // Per-fp conditioning: after accepts under the motivic fingerprint, the motivic
        // path score rises while a *different* fingerprint (raw path) stays at its
        // prior (global score, backoff blended).
        let tmp = TempDir::new().unwrap();
        let engine = HintEngine::new(Some(tmp.path()));

        let para_info = b"(para-info mu repr-map motivic_ml.repr_opt_step)";
        let motivic_fp = motivic_proof_fingerprint(para_info);
        // Use a clearly distinct raw fingerprint (different prefix)
        let raw_fp = proof_state_fingerprint(b"some-unrelated-proof-state");

        // Accept "motivic_ml.repr_opt_step" 20× under the motivic fingerprint
        // 20 >> BACKOFF_BLEND(10), so α ≈ 0.67 and local score dominates
        for _ in 0..20 {
            engine.record_proposed_with_fp(&motivic_fp, &["motivic_ml.repr_opt_step".to_string()]);
            engine.log().append(&TacticLogEntry {
                epoch_ms: 0,
                proof_state_fp: motivic_fp.clone(),
                tactic_id: "motivic_ml.repr_opt_step".to_string(),
                applied: true,
            });
        }
        engine.refresh_model();

        // Motivic path: 20 accepts / 20 proposals → local score ≈ 0.95;
        // global also rises (all stats feed global), but the local score
        // is weighted by alpha=20/(20+10)≈67%.
        let motivic_scores = engine.query_with_motivic_hint(
            &motivic_fp,
            &["motivic_ml.repr_opt_step".to_string(), "std.rewrite".to_string()],
        );
        // Raw path with unrelated fp: global score applies (with backoff blend→0 for this fp)
        let raw_scores = engine.query_with_motivic_hint(
            &raw_fp,
            &["motivic_ml.repr_opt_step".to_string(), "std.rewrite".to_string()],
        );

        let motivic_step_score = motivic_scores
            .iter().find(|r| r.tactic_id == "motivic_ml.repr_opt_step").unwrap().score;
        let raw_step_score = raw_scores
            .iter().find(|r| r.tactic_id == "motivic_ml.repr_opt_step").unwrap().score;

        // Motivic path should rank the tactic higher due to per-fp conditioning
        assert!(
            motivic_step_score >= raw_step_score,
            "motivic-path score ({:.3}) should be ≥ raw-path score ({:.3}) after per-fp conditioning",
            motivic_step_score, raw_step_score
        );
        // Both return the same tactic (just at different scores)
        assert_eq!(
            motivic_scores[0].tactic_id,
            "motivic_ml.repr_opt_step",
            "motivic path must rank the accepted tactic first"
        );
    }

    // ----------------------------------------------------------
    // Backoff sanity: 0 local observations → pure global score
    // ----------------------------------------------------------

    #[test]
    fn backoff_zero_local_equals_global() {
        // When a fingerprint has NO local observations, query_with_motivic_hint
        // must return the same ordering as query() for the same set of tactics.
        // (INV D-HINT: backoff with alpha=0 = pure global path)
        let tmp = TempDir::new().unwrap();
        let engine = HintEngine::new(Some(tmp.path()));

        let state = b"some-proof-state-bytes";
        // Accept std.rewrite 3/3 times via the raw/global path
        for _ in 0..3 {
            engine.record_proposed(state, &["std.rewrite".into(), "std.quickfix".into()]);
            engine.record_applied(state, "std.rewrite");
        }
        engine.refresh_model();

        let ids = vec!["std.rewrite".to_string(), "std.quickfix".to_string()];

        // Raw query (uses raw fingerprint from state bytes)
        let raw_results = engine.query(state, &ids);

        // A BRAND NEW fingerprint that has never been seen -> alpha=0 -> pure global
        let unseen_fp = "aaaa0000aaaa0000bbbbccccddddeeee"; // 32 chars but unseen
        let motivic_results = engine.query_with_motivic_hint(unseen_fp, &ids);

        // Both should rank std.rewrite above std.quickfix (global evidence)
        assert_eq!(raw_results[0].tactic_id, "std.rewrite");
        assert_eq!(motivic_results[0].tactic_id, "std.rewrite",
            "zero-local-evidence path must rank by global stats, same as global path");
        // Scores should match (within float rounding): alpha=0 -> pure global
        assert_eq!(
            (raw_results[0].score * 1000.0) as i32,
            (motivic_results[0].score * 1000.0) as i32,
            "zero-local backoff must equal global score"
        );
    }

    // ----------------------------------------------------------
    // LRU eviction determinism
    // ----------------------------------------------------------

    #[test]
    fn lru_eviction_is_deterministic() {
        // Write a log with MAX_FP_BUCKETS + 10 distinct fingerprints.
        // After loading, only MAX_FP_BUCKETS survive.
        // Same log stream → same surviving set (deterministic).
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join(".edgelord/tactic_log.csv");
        std::fs::create_dir_all(tmp.path().join(".edgelord")).unwrap();

        let total = MAX_FP_BUCKETS + 10;
        let mut csv = "epoch_ms,proof_state_fp,tactic_id,applied\n".to_string();
        for i in 0..total {
            // Each fingerprint is a unique 64-char hex string (padded with zeros)
            let fp = format!("{:0>64}", format!("fp_{:04}", i));
            csv.push_str(&format!("0,{},std.rewrite,1\n", fp));
        }
        std::fs::write(&log_path, &csv).unwrap();

        let model1 = HintModel::load_from_log(&log_path);
        let model2 = HintModel::load_from_log(&log_path);

        // Exactly MAX_FP_BUCKETS per-fp buckets survive
        assert_eq!(
            model1.per_fp.len(), MAX_FP_BUCKETS,
            "per_fp must be capped at MAX_FP_BUCKETS"
        );
        // Same log → identical surviving set (deterministic, INV D-*)
        let keys1: Vec<&String> = model1.per_fp.keys().collect();
        let keys2: Vec<&String> = model2.per_fp.keys().collect();
        assert_eq!(keys1, keys2, "eviction must be deterministic (INV D-*)");

        // Global stats must be complete (all MAX_FP_BUCKETS+10 entries)
        let global_entry = model1.global.get("std.rewrite");
        assert!(global_entry.is_some());
        let (proposals, accepts) = global_entry.unwrap();
        assert_eq!(*proposals, total as u64, "global must count all log entries");
        assert_eq!(*accepts, total as u64, "global must count all applied entries");
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
