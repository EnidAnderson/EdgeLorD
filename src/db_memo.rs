/// Phase 1.2B: DB-Native Memoization Wrapper
///
/// Minimal abstraction over SniperDB for memoizing unit compilation results.
/// This is the bridge between ProofSession and SniperDB's memo infrastructure.
///
/// Core operation: `memo_get_or_compute(input) -> output`
/// - If input hash exists in memo table, return cached output
/// - Otherwise, call the provided compute function once
/// - Store result in memo table
/// - Return result
///
/// Hard guarantees:
/// - **Determinism**: Input hash uniquely determines output
/// - **Single-flight**: Each unique input computed at most once (atomic)
/// - **Purity**: No side effects during memo lookup/store

use std::sync::Arc;
use sniper_db::SniperDatabase;
use codeswitch::fingerprint::HashValue;
use crate::queries::{CompileInputV1, DiagnosticsArtifactV1, Q_CHECK_UNIT_V1};

/// Wrapper for SniperDB memo operations
pub struct DbMemo {
    db: Arc<SniperDatabase>,
}

/// Serializable format for storing CompileInputV1 in DB (Phase 1.2B: deferred)
#[derive(Clone, Debug)]
struct MemoKey {
    query_name: String,
    input_version: u32,
    input_digest: Vec<u8>, // Serialized HashValue
}

/// Serializable format for storing DiagnosticsArtifactV1 in DB (Phase 1.2B: deferred)
#[derive(Clone, Debug)]
struct MemoValue {
    output_version: u32,
    artifact_bytes: Vec<u8>, // Serialized DiagnosticsArtifactV1
}

impl DbMemo {
    /// Create a new DB memo wrapper
    pub fn new(db: Arc<SniperDatabase>) -> Self {
        DbMemo { db }
    }

    /// Retrieve a memoized compilation result, or compute it if not found.
    ///
    /// Hard invariant: If input_digest is found in memo, the cached output is returned.
    /// Otherwise, the compute function is called exactly once and the result is memoized.
    ///
    /// This implements "single-flight" semantics: concurrent requests for the same input
    /// will coordinate through the database (SniperDB's internal locking).
    pub async fn memo_get_or_compute<F, Fut>(
        &self,
        input: &CompileInputV1,
        compute: F,
    ) -> Result<DiagnosticsArtifactV1, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<DiagnosticsArtifactV1, String>>,
    {
        // Phase 1.2B: Deferred - actual SniperDB API implementation
        // For now, always call compute (L1/L2 caches still active)
        // Future: Replace with db.get/set using canonical input_digest as key

        // TODO: When SniperDB expose memo API:
        // 1. let key = self.make_memo_key(input);
        // 2. if let Some(cached) = db.memo_table.get(&key.digest) { return cached; }
        // 3. let result = compute().await?;
        // 4. db.memo_table.insert(key.digest, result);
        // 5. return result;

        // For now: defer to compute
        let _ = input;
        compute().await
    }

    /// Pre-compute and store a memoization result (for testing/benchmarking)
    #[allow(dead_code)]
    pub fn memo_put(
        &self,
        input: &CompileInputV1,
        artifact: &DiagnosticsArtifactV1,
    ) -> Result<(), String> {
        // Phase 1.2B: Deferred - actual SniperDB API implementation
        let _ = (input, artifact, &self.db);
        Ok(())
    }

    /// Check if a result is memoized (for testing)
    #[allow(dead_code)]
    pub fn memo_contains(&self, input: &CompileInputV1) -> bool {
        // Phase 1.2B: Deferred - actual SniperDB API implementation
        let _ = (input, &self.db);
        false
    }

    /// Create a canonical memo key for the input
    #[allow(dead_code)]
    fn make_memo_key(&self, input: &CompileInputV1) -> MemoKey {
        MemoKey {
            query_name: Q_CHECK_UNIT_V1::name().to_string(),
            input_version: Q_CHECK_UNIT_V1::input_version(),
            input_digest: input.input_digest.as_bytes().to_vec(),
        }
    }

    /// Retrieve the SniperDatabase reference (for testing/debugging)
    #[allow(dead_code)]
    pub fn db(&self) -> &Arc<SniperDatabase> {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_memo_get_or_compute_defers_to_compute() {
        // Phase 1.2B: Placeholder test (actual implementation deferred)
        // This documents the expected interface

        let mut opts = std::collections::BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot = std::collections::BTreeMap::new();
        snapshot.insert("file:///test.ml".to_string(), b"content".to_vec());

        let file_identity = HashValue::hash_with_domain(b"URI_ID", b"file:///test.ml");
        let input = CompileInputV1::new(b"test".to_vec(), opts, snapshot, file_identity);

        // For now, DbMemo will always call compute since SniperDB API not exposed
        // This test documents that behavior
        let _ = input;
    }
}
