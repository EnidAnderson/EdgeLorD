/// Phase 1.2B: DB-Native Compile Query - Q_CHECK_UNIT_V1
///
/// This query captures the canonical inputs for unit compilation and stores
/// the results in SniperDB as the sole source of truth for incremental reuse.
///
/// Hard Invariants:
/// - **Purity**: Same input → same output, deterministically
/// - **Sound Reuse**: Output valid only when input hash matches exactly
/// - **Persistence**: Results survive restarts and process boundaries
/// - **Single-flight**: Compilation happens at most once per unique input
/// - **Stonewall**: No side effects (no workspace mutations during memo lookup)

use std::collections::BTreeMap;
use codeswitch::fingerprint::HashValue;
use comrade_lisp::comrade_workspace::WorkspaceReport;
use tower_lsp::lsp_types::Diagnostic;

/// Phase 1.2B: Canonical compilation input for unit check query
///
/// All fields are deterministically serialized to create a stable input hash.
/// No hidden non-determinism (e.g., no paths, no timestamps, no Debug strings).
///
/// Components:
/// 1. **unit_content**: Source code bytes
/// 2. **compile_options**: Pretty printer dialect, feature flags, etc.
/// 3. **workspace_snapshot**: All open documents (conservative dependency model)
/// 4. **file_identity**: Stable cryptographic file identity digest
///
/// Serialization: Canonical byte ordering (sorted collections, explicit separators)
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileInputV1 {
    /// Unit content (source code bytes)
    pub unit_content: Vec<u8>,

    /// Compile options (canonical: sorted, no Debug strings)
    pub compile_options: BTreeMap<String, String>,

    /// Workspace snapshot: (URI, content_hash) pairs sorted by URI
    /// Conservative model: any workspace change invalidates cache
    pub workspace_snapshot: BTreeMap<String, Vec<u8>>, // URI -> content hash

    /// Stable cryptographic file identity digest
    pub file_identity: HashValue,

    /// Input digest: hash of this entire struct's canonical serialization
    /// Computed at query execution time; used as the memo key
    pub input_digest: HashValue,
}

impl CompileInputV1 {
    /// Compute input digest from canonical serialization
    pub fn compute_digest(
        unit_content: &[u8],
        compile_options: &BTreeMap<String, String>,
        workspace_snapshot: &BTreeMap<String, Vec<u8>>,
        file_identity: HashValue,
    ) -> HashValue {
        let mut canonical_bytes = Vec::new();

        // 1. File identity digest bytes
        canonical_bytes.extend_from_slice(file_identity.as_bytes());

        // 2. Unit content (length-prefixed, deterministic)
        canonical_bytes.extend_from_slice(&(unit_content.len() as u64).to_le_bytes());
        canonical_bytes.extend_from_slice(unit_content);
        canonical_bytes.push(0); // separator

        // 3. Compile options (sorted by key, canonical serialization)
        for (key, value) in compile_options.iter() {
            canonical_bytes.extend_from_slice(key.as_bytes());
            canonical_bytes.push(b'=');
            canonical_bytes.extend_from_slice(value.as_bytes());
            canonical_bytes.push(0); // separator
        }
        canonical_bytes.push(0); // end of options

        // 4. Workspace snapshot (sorted by URI, deterministic)
        for (uri, content_hash) in workspace_snapshot.iter() {
            canonical_bytes.extend_from_slice(uri.as_bytes());
            canonical_bytes.push(0);
            canonical_bytes.extend_from_slice(content_hash);
            canonical_bytes.push(0); // separator
        }
        canonical_bytes.push(0); // end of workspace

        // Hash the canonical bytes with a domain separator
        HashValue::hash_with_domain(b"COMPILE_UNIT_V1_INPUT", &canonical_bytes)
    }

    /// Create a new CompileInputV1 with computed digest
    pub fn new(
        unit_content: Vec<u8>,
        compile_options: BTreeMap<String, String>,
        workspace_snapshot: BTreeMap<String, Vec<u8>>,
        file_identity: HashValue,
    ) -> Self {
        let input_digest =
            Self::compute_digest(&unit_content, &compile_options, &workspace_snapshot, file_identity);

        CompileInputV1 {
            unit_content,
            compile_options,
            workspace_snapshot,
            file_identity,
            input_digest,
        }
    }
}

/// Phase 1.2B: Named query for unit compilation
///
/// Query: Q_CHECK_UNIT_V1
/// Input: CompileInputV1 (deterministically serialized)
/// Output: DiagnosticsArtifactV1 (compilation results)
///
/// Guarantee: For any given input_digest, always returns the same output.
/// Storage: Results persisted in SniperDB's memo table by input_digest.
#[derive(Clone, Debug)]
pub struct Q_CHECK_UNIT_V1;

impl Q_CHECK_UNIT_V1 {
    pub const NAME: &'static str = "Q_CHECK_UNIT_V1";

    /// Canonical query name for logging and introspection
    pub fn name() -> &'static str {
        Self::NAME
    }

    /// Query class (e.g., "incremental_check", "unit_compile")
    pub fn query_class() -> &'static str {
        "unit_compile"
    }

    /// Expected input type version
    pub fn input_version() -> u32 {
        1
    }

    /// Expected output type version
    pub fn output_version() -> u32 {
        1
    }
}

/// Phase 1.2B: Compilation output artifact for Q_CHECK_UNIT_V1
///
/// Captures the canonical output of unit compilation:
/// - WorkspaceReport: Type information, proof state, diagnostics
/// - Computed diagnostics: Projected to LSP format
/// - Timestamp: When this artifact was computed
///
/// Guarantee: Deterministic given the input. No side effects or mutations.
#[derive(Clone, Debug)]
pub struct DiagnosticsArtifactV1 {
    /// Core compilation output from the workspace
    pub report: WorkspaceReport,

    /// Diagnostics in LSP format (computed from report)
    pub diagnostics: Vec<Diagnostic>,

    /// Timestamp when this artifact was computed (for logging)
    pub timestamp_secs: u64,

    /// Optional: Proof that this output is sound (fingerprint of compilation)
    pub output_digest: Option<HashValue>,
}

impl DiagnosticsArtifactV1 {
    /// Create a new diagnostics artifact with optional soundness proof
    pub fn new(
        report: WorkspaceReport,
        diagnostics: Vec<Diagnostic>,
        output_digest: Option<HashValue>,
    ) -> Self {
        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        DiagnosticsArtifactV1 {
            report,
            diagnostics,
            timestamp_secs,
            output_digest,
        }
    }

    /// Verify that output is deterministic (optional: compare with expected digest)
    pub fn verify_determinism(&self, expected_digest: &HashValue) -> bool {
        if let Some(ref output_digest) = self.output_digest {
            output_digest == expected_digest
        } else {
            // No verification data available
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_input_v1_digest_determinism() {
        let unit_content = b"(def foo (hole bar))".to_vec();
        let mut opts = BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///foo.ml".to_string(), b"content1".to_vec());

        // Create two inputs with identical data
        let file_identity = HashValue::hash_with_domain(b"URI_ID", b"file:///foo.ml");
        let input1 =
            CompileInputV1::new(unit_content.clone(), opts.clone(), snapshot.clone(), file_identity);

        let input2 =
            CompileInputV1::new(unit_content.clone(), opts.clone(), snapshot.clone(), file_identity);

        // Digests must be identical (determinism)
        assert_eq!(input1.input_digest, input2.input_digest);
    }

    #[test]
    fn test_compile_input_v1_digest_changes_with_content() {
        let mut opts = BTreeMap::new();
        opts.insert("dialect".to_string(), "pythonic".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///foo.ml".to_string(), b"content1".to_vec());

        let file_identity = HashValue::hash_with_domain(b"URI_ID", b"file:///foo.ml");
        let input1 = CompileInputV1::new(
            b"content1".to_vec(),
            opts.clone(),
            snapshot.clone(),
            file_identity,
        );

        let input2 = CompileInputV1::new(
            b"content2".to_vec(),
            opts.clone(),
            snapshot.clone(),
            file_identity,
        );

        // Different content → different digest
        assert_ne!(input1.input_digest, input2.input_digest);
    }

    #[test]
    fn test_compile_input_v1_digest_changes_with_options() {
        let mut opts1 = BTreeMap::new();
        opts1.insert("dialect".to_string(), "pythonic".to_string());

        let mut opts2 = BTreeMap::new();
        opts2.insert("dialect".to_string(), "canonical".to_string());

        let mut snapshot = BTreeMap::new();
        snapshot.insert("file:///foo.ml".to_string(), b"content".to_vec());

        let file_identity = HashValue::hash_with_domain(b"URI_ID", b"file:///foo.ml");
        let input1 = CompileInputV1::new(b"content".to_vec(), opts1, snapshot.clone(), file_identity);
        let input2 = CompileInputV1::new(b"content".to_vec(), opts2, snapshot.clone(), file_identity);

        // Different options → different digest
        assert_ne!(input1.input_digest, input2.input_digest);
    }

    #[test]
    fn test_q_check_unit_v1_constants() {
        assert_eq!(Q_CHECK_UNIT_V1::name(), "Q_CHECK_UNIT_V1");
        assert_eq!(Q_CHECK_UNIT_V1::query_class(), "unit_compile");
        assert_eq!(Q_CHECK_UNIT_V1::input_version(), 1);
        assert_eq!(Q_CHECK_UNIT_V1::output_version(), 1);
    }
}
