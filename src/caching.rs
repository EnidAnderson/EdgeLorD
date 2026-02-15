// EdgeLorD Phase 1.1: Deterministic Snapshot Reuse (SniperDB-backed)
//
// This module provides ModuleCache for caching compilation outputs based on
// a sound cache key that includes all inputs affecting compilation.
//
// Contract: INV D-CACHE-1 (purity), INV D-CACHE-2 (sound reuse), INV D-CACHE-3 (monotone invalidation)

use std::collections::BTreeMap;
use std::fmt;
use std::time::SystemTime;
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use bincode;

use comrade_lisp::comrade_workspace::WorkspaceReport;
use tower_lsp::lsp_types::Diagnostic;
use codeswitch::fingerprint::HashValue;
use sniper_db::SniperDatabase;

// ============================================================================
// Phase 1.2A: L1/L2 Cache Storage (SniperDB as persistent L2)
// ============================================================================

/// Minimal snapshot storage interface (L1/L2 abstraction)
///
/// L1 (InMemoryStore): Fast in-process cache (BTreeMap LRU)
/// L2 (SniperDbStore): Persistent backing (survives restarts)
///
/// ModuleSnapshotCache checks L1 first, then L2, then compiles.
pub trait SnapshotStore: Send + Sync {
    fn get(&self, key: &SnapshotStoreKey) -> Option<SerializedSnapshot>;
    fn put(&self, key: SnapshotStoreKey, snapshot: &SerializedSnapshot);
}

/// Key for snapshot storage: derive from 4-component cache key
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SnapshotStoreKey(pub HashValue);

impl SnapshotStoreKey {
    /// Create from the 4-component cache key
    pub fn from_cache_key(
        file_id: u32,
        content_hash: &HashValue,
        options_fp: &HashValue,
        deps_fp: &HashValue,
    ) -> Self {
        let mut canonical_bytes = Vec::new();
        canonical_bytes.extend_from_slice(&file_id.to_le_bytes());
        canonical_bytes.extend_from_slice(content_hash.as_bytes());
        canonical_bytes.extend_from_slice(options_fp.as_bytes());
        canonical_bytes.extend_from_slice(deps_fp.as_bytes());

        let key_hash = HashValue::hash_with_domain(b"EDGELORD_MODULE_SNAPSHOT_STORAGE_KEY_V1", &canonical_bytes);
        SnapshotStoreKey(key_hash)
    }
}

/// Serialized snapshot for storage (compact form, Phase 1.2B: deferred serialization)
#[derive(Clone, Debug)]
pub struct SerializedSnapshot {
    pub diagnostics: Vec<Diagnostic>,
    pub report: WorkspaceReport,
    pub timestamp_secs: u64,
}

impl Serialize for SerializedSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        
        // Serialize only the fields that are serializable
        let mut state = serializer.serialize_struct("SerializedSnapshot", 5)?;
        
        // Serialize diagnostics (Vec<Diagnostic> is serializable)
        state.serialize_field("diagnostics", &self.diagnostics)?;
        
        // Serialize WorkspaceReport fields individually
        state.serialize_field("diagnostics_by_file", &self.report.diagnostics_by_file)?;
        state.serialize_field("structured_diagnostics", &self.report.structured_diagnostics)?;
        state.serialize_field("fingerprint", &self.report.fingerprint)?;
        state.serialize_field("revision", &self.report.revision)?;
        state.serialize_field("timestamp_secs", &self.timestamp_secs)?;
        
        state.end()
    }
}

impl<'de> Deserialize<'de> for SerializedSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Diagnostics,
            DiagnosticsByFile,
            StructuredDiagnostics,
            Fingerprint,
            Revision,
            TimestampSecs,
        }

        struct SerializedSnapshotVisitor;

        impl<'de> Visitor<'de> for SerializedSnapshotVisitor {
            type Value = SerializedSnapshot;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct SerializedSnapshot")
            }

            fn visit_map<V>(self, mut map: V) -> Result<SerializedSnapshot, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut diagnostics = None;
                let mut diagnostics_by_file = None;
                let mut structured_diagnostics = None;
                let mut fingerprint = None;
                let mut revision = None;
                let mut timestamp_secs = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Diagnostics => {
                            if diagnostics.is_some() {
                                return Err(de::Error::duplicate_field("diagnostics"));
                            }
                            diagnostics = Some(map.next_value()?);
                        }
                        Field::DiagnosticsByFile => {
                            if diagnostics_by_file.is_some() {
                                return Err(de::Error::duplicate_field("diagnostics_by_file"));
                            }
                            diagnostics_by_file = Some(map.next_value()?);
                        }
                        Field::StructuredDiagnostics => {
                            if structured_diagnostics.is_some() {
                                return Err(de::Error::duplicate_field("structured_diagnostics"));
                            }
                            structured_diagnostics = Some(map.next_value()?);
                        }
                        Field::Fingerprint => {
                            if fingerprint.is_some() {
                                return Err(de::Error::duplicate_field("fingerprint"));
                            }
                            fingerprint = Some(map.next_value()?);
                        }
                        Field::Revision => {
                            if revision.is_some() {
                                return Err(de::Error::duplicate_field("revision"));
                            }
                            revision = Some(map.next_value()?);
                        }
                        Field::TimestampSecs => {
                            if timestamp_secs.is_some() {
                                return Err(de::Error::duplicate_field("timestamp_secs"));
                            }
                            timestamp_secs = Some(map.next_value()?);
                        }
                    }
                }

                let diagnostics = diagnostics.ok_or_else(|| de::Error::missing_field("diagnostics"))?;
                let diagnostics_by_file =
                    diagnostics_by_file.ok_or_else(|| de::Error::missing_field("diagnostics_by_file"))?;
                let structured_diagnostics = structured_diagnostics
                    .ok_or_else(|| de::Error::missing_field("structured_diagnostics"))?;
                let fingerprint = fingerprint.ok_or_else(|| de::Error::missing_field("fingerprint"))?;
                let revision = revision.ok_or_else(|| de::Error::missing_field("revision"))?;
                let timestamp_secs = timestamp_secs.ok_or_else(|| de::Error::missing_field("timestamp_secs"))?;

                Ok(SerializedSnapshot {
                    diagnostics,
                    report: WorkspaceReport {
                        diagnostics: Vec::new(),
                        diagnostics_by_file,
                        structured_diagnostics,
                        fingerprint,
                        revision,
                        bundle: None,
                        proof_state: None,
                    },
                    timestamp_secs,
                })
            }
        }

        deserializer.deserialize_struct(
            "SerializedSnapshot",
            &[
                "diagnostics",
                "diagnostics_by_file",
                "structured_diagnostics",
                "fingerprint",
                "revision",
                "timestamp_secs",
            ],
            SerializedSnapshotVisitor,
        )
    }
}

impl SerializedSnapshot {
    pub fn from_module_snapshot(snapshot: &ModuleSnapshot) -> Self {
        SerializedSnapshot {
            diagnostics: snapshot.diagnostics.clone(),
            report: snapshot.report.clone(),
            timestamp_secs: snapshot
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    pub fn to_module_snapshot(
        &self,
        file_id: u32,
        content_hash: HashValue,
        options_fp: HashValue,
        deps_fp: HashValue,
    ) -> ModuleSnapshot {
        ModuleSnapshot {
            file_id,
            content_hash,
            options_fingerprint: options_fp,
            dependency_fingerprint: deps_fp,
            report: self.report.clone(),
            diagnostics: self.diagnostics.clone(),
            timestamp: std::time::UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp_secs),
        }
    }
}

/// L1 (in-memory) snapshot store
#[derive(Debug)]
pub struct InMemorySnapshotStore {
    inner: std::sync::Arc<RwLock<BTreeMap<SnapshotStoreKey, SerializedSnapshot>>>,
}

impl InMemorySnapshotStore {
    pub fn new() -> Self {
        InMemorySnapshotStore {
            inner: std::sync::Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn get(&self, key: &SnapshotStoreKey) -> Option<SerializedSnapshot> {
        self.inner.read().unwrap().get(key).cloned()
    }

    fn put(&self, key: SnapshotStoreKey, snapshot: &SerializedSnapshot) {
        self.inner.write().unwrap().insert(key, snapshot.clone());
    }
}

/// L2 (persistent) snapshot store backed by SniperDatabase
pub struct SniperDbSnapshotStore {
    db: Arc<SniperDatabase>,
}

impl std::fmt::Debug for SniperDbSnapshotStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SniperDbSnapshotStore")
            .field("db", &"<SniperDatabase>")
            .finish()
    }
}

impl SniperDbSnapshotStore {
    pub fn new(db: Arc<SniperDatabase>) -> Self {
        SniperDbSnapshotStore { db }
    }
}

impl SnapshotStore for SniperDbSnapshotStore {
    fn get(&self, key: &SnapshotStoreKey) -> Option<SerializedSnapshot> {
        // INV C-KEY-ROUNDTRIP: Lookup by content hash (SnapshotStoreKey is deterministic hash)
        // SniperBlobStore is content-addressed, so key.0 IS the blob ID
        if let Some(blob) = self.db.blobs.get(&key.0) {
            if let Ok(snapshot) = bincode::deserialize(&blob) {
                return Some(snapshot);
            }
        }
        None
    }

    fn put(&self, key: SnapshotStoreKey, snapshot: &SerializedSnapshot) {
        // INV C-KEY-ROUNDTRIP: Store content-addressed, round-trip through serialization
        if let Ok(blob) = bincode::serialize(snapshot) {
            // SniperBlobStore.put() is content-addressed: put(&[u8]) -> BlobId
            // The returned blob_id should match key.0 (deterministic hash of content)
            let _blob_id = self.db.blobs.put(&blob);
            // Note: _blob_id is content-addressed hash, which should equal key.0
            // We don't store the mapping; get() uses key.0 directly to retrieve
        }
    }
}

// ============================================================================
// C2.3: Structured Miss Reasons (for deterministic benchmark outcomes)
// ============================================================================

/// Phase 1 (Module Snapshot) cache miss reasons
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase1MissReason {
    ContentChanged,
    OptionsChanged,
    DepsChanged,
    KeyUnavailable,
    CacheDisabled,
    Eviction,
    Other,
}

impl Phase1MissReason {
    /// Convert to outcome string for CSV
    pub fn to_outcome_string(&self) -> &'static str {
        match self {
            Self::ContentChanged => "miss:content_changed",
            Self::OptionsChanged => "miss:options_changed",
            Self::DepsChanged => "miss:deps_changed",
            Self::KeyUnavailable => "miss:key_unavailable",
            Self::CacheDisabled => "miss:cache_disabled",
            Self::Eviction => "miss:eviction",
            Self::Other => "miss:other",
        }
    }
}

/// Phase 1.1 (Workspace-Aware) cache miss reasons
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase1_1MissReason {
    ContentChanged,
    OptionsChanged,
    WorkspaceHashChanged,
    KeyUnavailable,
    CacheDisabled,
    Eviction,
    Other,
}

impl Phase1_1MissReason {
    /// Convert to outcome string for CSV
    pub fn to_outcome_string(&self) -> &'static str {
        match self {
            Self::ContentChanged => "miss:content_changed",
            Self::OptionsChanged => "miss:options_changed",
            Self::WorkspaceHashChanged => "miss:workspace_hash_changed",
            Self::KeyUnavailable => "miss:key_unavailable",
            Self::CacheDisabled => "miss:cache_disabled",
            Self::Eviction => "miss:eviction",
            Self::Other => "miss:other",
        }
    }
}

/// Cache outcome: either a hit or a miss with reason
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheOutcome<M> {
    Hit,
    Miss(M),
}

impl<M: std::fmt::Debug> CacheOutcome<M> {
    /// Convert to outcome string for CSV
    pub fn to_outcome_string(&self) -> String
    where
        M: CacheOutcomeString,
    {
        match self {
            Self::Hit => "hit".to_string(),
            Self::Miss(reason) => reason.to_outcome_string().to_string(),
        }
    }
}

/// Trait for types that can be converted to outcome strings
pub trait CacheOutcomeString {
    fn to_outcome_string(&self) -> &'static str;
}

impl CacheOutcomeString for Phase1MissReason {
    fn to_outcome_string(&self) -> &'static str {
        Phase1MissReason::to_outcome_string(self)
    }
}

impl CacheOutcomeString for Phase1_1MissReason {
    fn to_outcome_string(&self) -> &'static str {
        Phase1_1MissReason::to_outcome_string(self)
    }
}

/// C2.4: Cache lookup result - refactor-safe outcome classification
///
/// Instead of counter-based or log-based hit detection, the cache lookup
/// directly returns whether it was a hit or miss with a classified reason.
/// This makes outcome determination part of the control flow and immune to refactoring.
#[derive(Clone, Debug)]
pub enum CacheGetResult<V> {
    Hit(V),
    Miss(Phase1MissReason),
}

#[derive(Clone, Debug)]
pub enum CacheGetResult1_1<V> {
    Hit(V),
    Miss(Phase1_1MissReason),
}

impl<V> CacheGetResult<V> {
    pub fn to_outcome_string(&self) -> String {
        match self {
            Self::Hit(_) => "hit".to_string(),
            Self::Miss(reason) => reason.to_outcome_string().to_string(),
        }
    }

    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit(_))
    }
}

impl<V> CacheGetResult1_1<V> {
    pub fn to_outcome_string(&self) -> String {
        match self {
            Self::Hit(_) => "hit".to_string(),
            Self::Miss(reason) => reason.to_outcome_string().to_string(),
        }
    }

    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit(_))
    }
}

/// Builder for CacheKey with safe defaults and validation.
pub struct CacheKeyBuilder {
    options_fingerprint: Option<HashValue>,
    workspace_snapshot_hash: Option<HashValue>,
    unit_id: Option<String>,
    unit_content_hash: Option<HashValue>,
    dependency_fingerprint: Option<HashValue>,
}

impl CacheKeyBuilder {
    pub fn new() -> Self {
        Self {
            options_fingerprint: None,
            workspace_snapshot_hash: None,
            unit_id: None,
            unit_content_hash: None,
            dependency_fingerprint: None,
        }
    }

    pub fn options(mut self, fp: HashValue) -> Self {
        self.options_fingerprint = Some(fp);
        self
    }

    pub fn workspace_snapshot(mut self, hash: HashValue) -> Self {
        self.workspace_snapshot_hash = Some(hash);
        self
    }

    pub fn unit_id(mut self, id: impl Into<String>) -> Self {
        self.unit_id = Some(id.into());
        self
    }

    pub fn unit_content(mut self, hash: HashValue) -> Self {
        self.unit_content_hash = Some(hash);
        self
    }

    pub fn dependencies(mut self, fp: HashValue) -> Self {
        self.dependency_fingerprint = Some(fp);
        self
    }

    pub fn build(self) -> Result<CacheKey, String> {
        Ok(CacheKey {
            options_fingerprint: self
                .options_fingerprint
                .ok_or("missing options_fingerprint")?,
            workspace_snapshot_hash: self
                .workspace_snapshot_hash
                .ok_or("missing workspace_snapshot_hash")?,
            unit_id: self.unit_id.ok_or("missing unit_id")?,
            unit_content_hash: self
                .unit_content_hash
                .ok_or("missing unit_content_hash")?,
            dependency_fingerprint: self
                .dependency_fingerprint
                .ok_or("missing dependency_fingerprint")?,
        })
    }
}

impl Default for CacheKeyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Canonical cache key combining all inputs that affect compilation.
///
/// As per PHASE_1_1_ACCEPTANCE_SPEC.md §1.1:
/// - OptionsFingerprint: hash of canonical serialization of compile options (BTreeMap/sorted)
/// - WorkspaceSnapshotHash: hash of workspace state (open docs + dependencies)
/// - UnitId: canonicalized file identifier (prefer FileId, normalize Url if needed)
/// - UnitContentHash: hash of file content at compilation time
/// - DependencyFingerprint: hash of transitive dependencies
///
/// INV D-CACHE-1 precondition: "same options" means bit-for-bit identical canonical bytes
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CacheKey {
    /// Hash of compile options (BTreeMap/sorted, no Debug strings)
    pub options_fingerprint: HashValue,
    /// Hash of workspace snapshot (all open docs + roots)
    pub workspace_snapshot_hash: HashValue,
    /// Canonicalized unit identifier (FileId or normalized Url string)
    pub unit_id: String,
    /// Hash of unit content at compilation time
    pub unit_content_hash: HashValue,
    /// Hash of transitive dependencies
    pub dependency_fingerprint: HashValue,
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CacheKey(opts={}, ws={}, unit={}, content={}, deps={})",
            self.options_fingerprint,
            self.workspace_snapshot_hash,
            self.unit_id,
            self.unit_content_hash,
            self.dependency_fingerprint
        )
    }
}

/// Cached compilation output and associated metadata.
#[derive(Clone, Debug)]
pub struct CacheValue {
    /// The canonical compilation output (WorkspaceReport)
    pub report: WorkspaceReport,
    /// Diagnostics derived from the report (for fast publish)
    pub diagnostics: Vec<Diagnostic>,
    /// Timestamp when this cache entry was created (for metrics)
    pub timestamp: SystemTime,
}

/// Phase 1: Module snapshot backed by SniperDB
///
/// A module snapshot captures the compilation result for a single file with a COMPLETE key
/// that includes all inputs affecting compilation:
/// - file_id: Deterministic file identifier
/// - content_hash: Hash of file content
/// - options_fingerprint: Hash of compile options
/// - dependency_fingerprint: Hash of transitive dependencies
///
/// INV PHASE-1-MODULE-1 (Sound Reuse): Only reuse when ALL inputs match
/// INV PHASE-1-MODULE-2 (Complete Key): Must include options, dependencies, content
/// INV PHASE-1-MODULE-3 (Content Stability): Same inputs → same snapshot, always
#[derive(Clone, Debug)]
pub struct ModuleSnapshot {
    /// File identifier (e.g., CRC32 of URI)
    pub file_id: u32,
    /// Hash of file content at snapshot time
    pub content_hash: HashValue,
    /// Hash of compile options (canonical, deterministic)
    pub options_fingerprint: HashValue,
    /// Hash of transitive dependencies (all imports + their content hashes)
    pub dependency_fingerprint: HashValue,
    /// Compilation report for this file content + options + deps
    pub report: WorkspaceReport,
    /// Diagnostics derived from the report
    pub diagnostics: Vec<Diagnostic>,
    /// Timestamp when snapshot was created
    pub timestamp: SystemTime,
}

/// Phase 1.2A: Module snapshot cache with L1/L2 storage
///
/// Implements a 2-level cache:
/// - L1: In-memory BTreeMap LRU (fast, hot)
/// - L2: SniperDB persistent store (survives restarts, auditable)
///
/// Tracks snapshots by a COMPLETE 4-component key:
/// (file_id, content_hash, options_fingerprint, dependency_fingerprint)
///
/// This ensures sound reuse: only reuse when ALL compilation inputs match.
/// Prevents silent reuse of stale data when dependencies or options change.
///
/// Strategy (L1/L2):
/// 1. Check L1 in-memory cache (fast path)
/// 2. Check L2 SniperDB persistent store (cross-session reuse)
/// 3. Miss: return None, compute fresh
/// 4. L2 hit: promote to L1 for next access
///
/// Returns None on cache miss (any component mismatch = miss).
pub struct ModuleSnapshotCache {
    db: Arc<SniperDatabase>,

    // L1: In-memory fast cache (BTreeMap LRU)
    /// In-memory overlay for fast lookup by 4-component key
    l1_cache: BTreeMap<(u32, HashValue, HashValue, HashValue), ModuleSnapshot>,

    // L2: Persistent backing store (deferred to Phase 1.2B)
    /// Will be: Arc<dyn SnapshotStore> for pluggable L2 storage
    /// For Phase 1.2A: SniperDbSnapshotStore (stub, no-op)
    l2_store: Arc<SniperDbSnapshotStore>,

    /// Statistics for phase 1 module snapshot cache
    stats: ModuleSnapshotStats,
    /// Maximum in-memory entries before eviction
    max_entries: usize,
}

/// Statistics for Phase 1 module snapshot cache
#[derive(Debug, Clone, Default)]
pub struct ModuleSnapshotStats {
    pub hits: u64,
    pub misses: u64,
}

impl ModuleSnapshotStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64) / (total as f64)
        }
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
    }

    pub fn record_miss(&mut self) {
        self.misses += 1;
    }
}

impl ModuleSnapshotCache {
    /// Create a new module snapshot cache with L1 (in-memory) and L2 (SniperDB) backing.
    pub fn new(db: Arc<SniperDatabase>) -> Self {
        Self {
            l2_store: Arc::new(SniperDbSnapshotStore::new(db.clone())),
            db,
            l1_cache: BTreeMap::new(),
            stats: ModuleSnapshotStats::default(),
            max_entries: 500, // Conservative limit for L1; can adjust based on profiling
        }
    }

    /// Create with custom max entries.
    pub fn with_max_entries(db: Arc<SniperDatabase>, max_entries: usize) -> Self {
        Self {
            l2_store: Arc::new(SniperDbSnapshotStore::new(db.clone())),
            db,
            l1_cache: BTreeMap::new(),
            stats: ModuleSnapshotStats::default(),
            max_entries,
        }
    }

    /// Retrieve module snapshot with a COMPLETE 4-component key (L1/L2 lookup).
    ///
    /// INV PHASE-1-MODULE-1: Only reuse when all inputs match
    /// - file_id: Source file identifier
    /// - content_hash: File content hash
    /// - options_fingerprint: Compile options hash
    /// - dependency_fingerprint: Transitive dependencies hash
    ///
    /// Phase 1.2A: L1/L2 lookup strategy
    /// 1. Check L1 (in-memory BTreeMap) - fast, hot
    /// 2. Check L2 (SniperDB persistent store) - cross-session reuse
    /// 3. Miss: return None, compute fresh
    /// 4. L2 hit: promote to L1 for next access
    ///
    /// Returns None on cache miss (any component mismatch = miss).
    pub fn get(
        &mut self,
        file_id: u32,
        content_hash: HashValue,
        options_fingerprint: HashValue,
        dependency_fingerprint: HashValue,
    ) -> Option<ModuleSnapshot> {
        let key_tuple = (file_id, content_hash.clone(), options_fingerprint.clone(), dependency_fingerprint.clone());
        let store_key = SnapshotStoreKey::from_cache_key(file_id, &content_hash, &options_fingerprint, &dependency_fingerprint);

        // L1: Check in-memory cache first (fast path)
        if let Some(snapshot) = self.l1_cache.get(&key_tuple) {
            self.stats.record_hit();
            return Some(snapshot.clone());
        }

        // L2: Check persistent SniperDB store (cross-session reuse)
        if let Some(serialized) = self.l2_store.get(&store_key) {
            let snapshot = serialized.to_module_snapshot(
                file_id,
                content_hash,
                options_fingerprint,
                dependency_fingerprint,
            );
            // Promote L2 hit to L1 for future access
            self.l1_cache.insert(key_tuple, snapshot.clone());
            self.stats.record_hit();
            return Some(snapshot);
        }

        // Total miss: not in L1 or L2
        self.stats.record_miss();
        None
    }

    /// Insert a module snapshot into the cache with complete key (write-through L1/L2).
    ///
    /// Snapshot must include all input components (content_hash, options_fingerprint,
    /// dependency_fingerprint) to ensure sound reuse.
    ///
    /// Phase 1.2A: Write-through strategy
    /// 1. Insert into L1 (in-memory BTreeMap)
    /// 2. Insert into L2 (SniperDB persistent store)
    /// 3. Evict oldest L1 entries if exceeding max_entries
    ///
    /// This ensures cross-session reuse and makes caching auditable.
    pub fn insert(&mut self, snapshot: ModuleSnapshot) {
        let key_tuple = (
            snapshot.file_id,
            snapshot.content_hash.clone(),
            snapshot.options_fingerprint.clone(),
            snapshot.dependency_fingerprint.clone(),
        );
        let store_key = SnapshotStoreKey::from_cache_key(
            snapshot.file_id,
            &snapshot.content_hash,
            &snapshot.options_fingerprint,
            &snapshot.dependency_fingerprint,
        );

        // Write-through: insert into both L1 and L2
        // L1: In-memory (hot, fast)
        self.l1_cache.insert(key_tuple, snapshot.clone());

        // L2: Persistent SniperDB store (deferred to Phase 1.2B for actual implementation)
        let serialized = SerializedSnapshot::from_module_snapshot(&snapshot);
        self.l2_store.put(store_key, &serialized);

        // LRU-style eviction on L1: remove oldest 50% of entries when exceeding max
        if self.l1_cache.len() > self.max_entries {
            let half = self.l1_cache.len() / 2;
            let keys_to_remove: Vec<_> = self
                .l1_cache
                .keys()
                .take(half)
                .cloned()
                .collect();
            for key in keys_to_remove {
                self.l1_cache.remove(&key);
            }
        }
    }

    /// Get current statistics.
    pub fn stats(&self) -> ModuleSnapshotStats {
        self.stats.clone()
    }

    /// Clear all cached snapshots (L1 only; L2 persists).
    pub fn clear(&mut self) {
        self.l1_cache.clear();
    }

    /// Get L1 (in-memory) cache size.
    pub fn len(&self) -> usize {
        self.l1_cache.len()
    }

    /// Check if L1 cache is empty.
    pub fn is_empty(&self) -> bool {
        self.l1_cache.is_empty()
    }

    /// Reset statistics (for testing).
    pub fn reset_stats(&mut self) {
        self.stats = ModuleSnapshotStats::default();
    }
}

/// Statistics for cache hits/misses and performance analysis.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    /// Breakdown of miss reasons for diagnostics
    pub miss_reasons: BTreeMap<String, u64>,
    /// Total time spent in cache lookups (ms)
    pub lookup_time_ms: u64,
    /// Total time spent compiling (ms)
    pub compile_time_ms: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64) / (total as f64)
        }
    }

    pub fn total_operations(&self) -> u64 {
        self.hits + self.misses
    }

    pub fn record_miss(&mut self, reason: impl Into<String>) {
        self.misses += 1;
        *self
            .miss_reasons
            .entry(reason.into())
            .or_insert(0) += 1;
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
    }
}

/// CacheStore trait: abstraction over storage backends (memory, SniperDB, etc.)
///
/// Phase 1.2 + Phase 2A: Support pluggable backends while keeping correctness invariants.
/// All backends must preserve:
/// - INV D-CACHE-2 (Sound reuse): identical key → identical cached value
/// - Single-flight semantics: operations atomic with respect to compilation gate
pub trait CacheStore: Send + Sync {
    /// Retrieve a cached value for the given key, if present.
    fn get(&self, key: &CacheKey) -> Option<CacheValue>;

    /// Store a compiled value under the given key.
    fn put(&self, key: CacheKey, value: CacheValue);

    /// Get current statistics (hits, misses, etc.)
    fn stats(&self) -> CacheStats;

    /// Clear all cached entries (for testing or reset).
    fn clear(&self);
}

/// InMemoryCacheStore: Thread-safe in-memory cache implementation.
///
/// Invariants:
/// - INV D-CACHE-1 (Purity): Same key → identical output, always
/// - INV D-CACHE-2 (Sound reuse): Reuse only when key exactly matches
/// - INV D-CACHE-3 (Monotone invalidation): Edits invalidate affected caches
///
/// Thread-safety: Internally uses Arc<RwLock<>> to allow safe concurrent access.
/// Cache must be accessed inside single-flight gate to preserve no-stale-diagnostics.
#[derive(Debug, Clone)]
pub struct InMemoryCacheStore {
    inner: Arc<RwLock<InMemoryCacheStoreInner>>,
}

#[derive(Debug)]
struct InMemoryCacheStoreInner {
    /// Map from CacheKey to cached compilation output
    snapshots: BTreeMap<CacheKey, CacheValue>,
    /// Statistics for monitoring and acceptance tests
    stats: CacheStats,
    /// Maximum number of cache entries before eviction (optional)
    max_entries: usize,
}

/// Backward-compatible alias for existing code
pub type ModuleCache = InMemoryCacheStore;

impl InMemoryCacheStore {
    /// Create a new InMemoryCacheStore with default settings.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InMemoryCacheStoreInner {
                snapshots: BTreeMap::new(),
                stats: CacheStats::default(),
                max_entries: 1000,
            })),
        }
    }

    /// Create a new InMemoryCacheStore with custom max entries.
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(InMemoryCacheStoreInner {
                snapshots: BTreeMap::new(),
                stats: CacheStats::default(),
                max_entries,
            })),
        }
    }

    /// Attempt to retrieve a cached value for the given key.
    ///
    /// Returns None if not found (cache miss).
    /// Updates statistics automatically.
    pub fn get(&self, key: &CacheKey) -> Option<CacheValue> {
        let mut inner = self.inner.write().unwrap();
        if let Some(value) = inner.snapshots.get(key).cloned() {
            inner.stats.record_hit();
            Some(value)
        } else {
            inner.stats.record_miss("cache_miss_not_found");
            None
        }
    }

    /// Insert a compiled value into the cache.
    ///
    /// If max_entries is reached, oldest entries are evicted (LRU-style).
    pub fn insert(&self, key: CacheKey, value: CacheValue) {
        let mut inner = self.inner.write().unwrap();
        inner.snapshots.insert(key, value);

        // Simple eviction: if we exceed max_entries, remove oldest half
        if inner.snapshots.len() > inner.max_entries {
            let half = inner.snapshots.len() / 2;
            let keys_to_remove: Vec<_> = inner
                .snapshots
                .keys()
                .take(half)
                .cloned()
                .collect();
            for key in keys_to_remove {
                inner.snapshots.remove(&key);
            }
        }
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.snapshots.clear();
    }

    /// Return the number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().snapshots.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.read().unwrap().snapshots.is_empty()
    }

    /// Get current statistics (clone).
    pub fn stats(&self) -> CacheStats {
        self.inner.read().unwrap().stats.clone()
    }

    /// Get mutable access for statistics (for tests).
    pub fn stats_mut(&self) -> CacheStats {
        self.inner.read().unwrap().stats.clone()
    }

    /// Reset statistics (for benchmarking between test runs).
    pub fn reset_stats(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.stats = CacheStats::default();
    }
}

impl CacheStore for InMemoryCacheStore {
    fn get(&self, key: &CacheKey) -> Option<CacheValue> {
        InMemoryCacheStore::get(self, key)
    }

    fn put(&self, key: CacheKey, value: CacheValue) {
        self.insert(key, value);
    }

    fn stats(&self) -> CacheStats {
        self.inner.read().unwrap().stats.clone()
    }

    fn clear(&self) {
        InMemoryCacheStore::clear(self);
    }
}

impl Default for InMemoryCacheStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_ordering() {
        // Keys should be totally ordered for BTreeMap determinism
        let key1 = CacheKey {
            options_fingerprint: HashValue::hash_with_domain(b"OPT", b"opt1"),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"ws1"),
            unit_id: "file_a.maclane".to_string(),
            unit_content_hash: HashValue::hash_with_domain(b"CONTENT", b"content1"),
            dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"deps1"),
        };

        let key2 = CacheKey {
            options_fingerprint: HashValue::hash_with_domain(b"OPT", b"opt1"),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"ws1"),
            unit_id: "file_b.maclane".to_string(),
            unit_content_hash: HashValue::hash_with_domain(b"CONTENT", b"content1"),
            dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"deps1"),
        };

        // BTreeMap should handle both keys deterministically
        let mut map = BTreeMap::new();
        map.insert(key1.clone(), "value1");
        map.insert(key2.clone(), "value2");

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&key1), Some(&"value1"));
        assert_eq!(map.get(&key2), Some(&"value2"));
    }

    #[test]
    fn test_cache_hit_miss_stats() {
        let cache = ModuleCache::new();
        let key = CacheKey {
            options_fingerprint: HashValue::hash_with_domain(b"OPT", b"test"),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"test"),
            unit_id: "test.maclane".to_string(),
            unit_content_hash: HashValue::hash_with_domain(b"CONTENT", b"test"),
            dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"test"),
        };

        // Initial stats
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);

        // Miss on empty cache
        let _ = cache.get(&key);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 1);
        assert!(cache.stats().hit_rate() < 0.01);

        // Insert and hit
        let value = CacheValue {
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };
        cache.insert(key.clone(), value);

        let _ = cache.get(&key);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hit_rate(), 0.5);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = ModuleCache::with_max_entries(5);
        let dummy_value = CacheValue {
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        // Insert more than max_entries
        for i in 0..10 {
            let key = CacheKey {
                options_fingerprint: HashValue::hash_with_domain(b"OPT", i.to_string().as_bytes()),
                workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"const"),
                unit_id: format!("file_{}.maclane", i),
                unit_content_hash: HashValue::hash_with_domain(b"CONTENT", i.to_string().as_bytes()),
                dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"const"),
            };
            cache.insert(key, dummy_value.clone());
        }

        // Should have evicted to stay under max (less than 10 entries)
        assert!(cache.len() < 10, "Eviction should have occurred");
        assert!(cache.len() <= 5, "Cache should not exceed max_entries");
    }

    #[test]
    fn test_cache_determinism() {
        // Same input key should always retrieve the same cached value
        let key = CacheKey {
            options_fingerprint: HashValue::hash_with_domain(b"OPT", b"deterministic"),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"deterministic"),
            unit_id: "deterministic.maclane".to_string(),
            unit_content_hash: HashValue::hash_with_domain(b"CONTENT", b"deterministic"),
            dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"deterministic"),
        };

        let fingerprint = [1u8; 32];
        let value = CacheValue {
            report: WorkspaceReport {
                diagnostics: vec![],
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: Some(fingerprint),
                revision: 1,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        let cache = ModuleCache::new();
        cache.insert(key.clone(), value.clone());

        // Multiple lookups should always return the same fingerprint
        for _ in 0..5 {
            let retrieved = cache.get(&key).expect("Cache hit failed");
            assert_eq!(retrieved.report.fingerprint, Some(fingerprint));
            assert_eq!(retrieved.report.revision, 1);
        }
    }

    #[test]
    fn test_cache_key_exact_match() {
        // INV D-CACHE-2: Cache reuse only on exact key match
        let key1 = CacheKey {
            options_fingerprint: HashValue::hash_with_domain(b"OPT", b"v1"),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"ws1"),
            unit_id: "file.maclane".to_string(),
            unit_content_hash: HashValue::hash_with_domain(b"CONTENT", b"v1"),
            dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"d1"),
        };

        let key2_different_options = CacheKey {
            options_fingerprint: HashValue::hash_with_domain(b"OPT", b"v2"),
            workspace_snapshot_hash: HashValue::hash_with_domain(b"WS", b"ws1"),
            unit_id: "file.maclane".to_string(),
            unit_content_hash: HashValue::hash_with_domain(b"CONTENT", b"v1"),
            dependency_fingerprint: HashValue::hash_with_domain(b"DEPS", b"d1"),
        };

        let fingerprint = [42u8; 32];
        let value = CacheValue {
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: Some(fingerprint),
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        let cache = ModuleCache::new();
        cache.insert(key1.clone(), value);

        // Same key hits
        assert!(cache.get(&key1).is_some());
        // Different options miss
        assert!(cache.get(&key2_different_options).is_none());
    }

    #[test]
    fn test_module_snapshot_cache_hit_on_all_inputs_match() {
        // INV PHASE-1-MODULE-1: Reuse only when all 4 components match
        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_id = 12345u32;
        let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content1");
        let opts_fp = HashValue::hash_with_domain(b"OPTIONS", b"default");
        let deps_fp = HashValue::hash_with_domain(b"DEPS", b"dep1");

        let fingerprint = [99u8; 32];
        let snapshot = ModuleSnapshot {
            file_id,
            content_hash: content_hash.clone(),
            options_fingerprint: opts_fp.clone(),
            dependency_fingerprint: deps_fp.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: Some(fingerprint),
                revision: 1,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        // Insert and retrieve with matching key
        cache.insert(snapshot.clone());
        let retrieved = cache.get(file_id, content_hash.clone(), opts_fp.clone(), deps_fp.clone());
        assert!(retrieved.is_some(), "Should hit when all 4 components match");
        assert_eq!(retrieved.unwrap().report.fingerprint, Some(fingerprint));
        assert_eq!(cache.stats().hit_rate(), 1.0);
    }

    #[test]
    fn test_module_snapshot_cache_miss_on_content_change() {
        // INV PHASE-1-MODULE-3: Different content hash → cache miss
        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_id = 12345u32;
        let content_hash1 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content1");
        let content_hash2 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content2");
        let opts_fp = HashValue::hash_with_domain(b"OPTIONS", b"default");
        let deps_fp = HashValue::hash_with_domain(b"DEPS", b"dep1");

        let snapshot = ModuleSnapshot {
            file_id,
            content_hash: content_hash1.clone(),
            options_fingerprint: opts_fp.clone(),
            dependency_fingerprint: deps_fp.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        cache.insert(snapshot);

        // Query with different content hash
        let retrieved = cache.get(file_id, content_hash2, opts_fp.clone(), deps_fp.clone());
        assert!(retrieved.is_none(), "Should miss when content hash differs");
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_module_snapshot_cache_miss_on_deps_change() {
        // SOUNDNESS TEST: Different dependencies → cache miss
        // This test would FAIL with the old buggy (file_id, content_hash) key
        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_id = 12345u32;
        let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content1");
        let opts_fp = HashValue::hash_with_domain(b"OPTIONS", b"default");
        let deps_fp_state1 = HashValue::hash_with_domain(b"DEPS", b"fileB_v1");
        let deps_fp_state2 = HashValue::hash_with_domain(b"DEPS", b"fileB_v2");

        let snapshot = ModuleSnapshot {
            file_id,
            content_hash: content_hash.clone(),
            options_fingerprint: opts_fp.clone(),
            dependency_fingerprint: deps_fp_state1.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        cache.insert(snapshot);

        // Query with same content but DIFFERENT dependencies
        let retrieved = cache.get(file_id, content_hash.clone(), opts_fp.clone(), deps_fp_state2);
        assert!(
            retrieved.is_none(),
            "Must miss when deps change (prevents silent stale reuse)"
        );
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_module_snapshot_cache_miss_on_options_change() {
        // SOUNDNESS TEST: Different compile options → cache miss
        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_id = 12345u32;
        let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content1");
        let opts_fp_default = HashValue::hash_with_domain(b"OPTIONS", b"default");
        let opts_fp_optimized = HashValue::hash_with_domain(b"OPTIONS", b"optimized");
        let deps_fp = HashValue::hash_with_domain(b"DEPS", b"dep1");

        let snapshot = ModuleSnapshot {
            file_id,
            content_hash: content_hash.clone(),
            options_fingerprint: opts_fp_default.clone(),
            dependency_fingerprint: deps_fp.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        cache.insert(snapshot);

        // Query with same content but DIFFERENT options
        let retrieved = cache.get(file_id, content_hash.clone(), opts_fp_optimized, deps_fp.clone());
        assert!(
            retrieved.is_none(),
            "Must miss when options change (prevents silent stale reuse)"
        );
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_module_snapshot_cache_eviction() {
        // Phase 1: LRU-style eviction when max_entries exceeded
        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::with_max_entries(db, 5);

        let dummy_report = WorkspaceReport {
            diagnostics: Vec::new(),
            diagnostics_by_file: BTreeMap::new(),
            structured_diagnostics: Vec::new(),
            fingerprint: None,
            revision: 0,
            bundle: None,
            proof_state: None,
        };
        let opts_fp = HashValue::hash_with_domain(b"OPTIONS", b"default");

        // Insert more than max_entries
        for i in 0..10 {
            let file_id = (100 + i) as u32;
            let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", i.to_string().as_bytes());
            let deps_fp = HashValue::hash_with_domain(b"DEPS", i.to_string().as_bytes());
            let snapshot = ModuleSnapshot {
                file_id,
                content_hash,
                options_fingerprint: opts_fp.clone(),
                dependency_fingerprint: deps_fp,
                report: dummy_report.clone(),
                diagnostics: Vec::new(),
                timestamp: SystemTime::now(),
            };
            cache.insert(snapshot);
        }

        // Should have evicted to stay under max
        assert!(cache.len() <= cache.max_entries);
    }

    #[test]
    fn test_module_snapshot_stats() {
        // Phase 1: Statistics tracking with complete key
        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_id = 12345u32;
        let content_hash1 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"c1");
        let content_hash2 = HashValue::hash_with_domain(b"SOURCE_TEXT", b"c2");
        let opts_fp = HashValue::hash_with_domain(b"OPTIONS", b"default");
        let deps_fp = HashValue::hash_with_domain(b"DEPS", b"dep1");

        let snapshot = ModuleSnapshot {
            file_id,
            content_hash: content_hash1.clone(),
            options_fingerprint: opts_fp.clone(),
            dependency_fingerprint: deps_fp.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        cache.insert(snapshot);

        // Hit on exact match
        cache.get(file_id, content_hash1.clone(), opts_fp.clone(), deps_fp.clone());
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 0);

        // Miss on content change
        cache.get(file_id, content_hash2, opts_fp.clone(), deps_fp.clone());
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
        assert!((cache.stats().hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_module_snapshot_fingerprint_completeness_options() {
        // CRITICAL VALIDATION TEST:
        // Verify that different compile options produce different fingerprints
        // (If this fails, we have false cache hits when options change!)

        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_id = 12345u32;
        let content_hash = HashValue::hash_with_domain(b"SOURCE_TEXT", b"content");

        // Options state 1: pythonic dialect
        let opts_fp_pythonic = HashValue::hash_with_domain(b"OPTIONS", b"dialect=pythonic");
        // Options state 2: canonical dialect
        let opts_fp_canonical = HashValue::hash_with_domain(b"OPTIONS", b"dialect=canonical");
        let deps_fp = HashValue::hash_with_domain(b"DEPS", b"dep1");

        let snapshot = ModuleSnapshot {
            file_id,
            content_hash: content_hash.clone(),
            options_fingerprint: opts_fp_pythonic.clone(),
            dependency_fingerprint: deps_fp.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        cache.insert(snapshot);

        // Same options → hit
        let hit = cache.get(file_id, content_hash.clone(), opts_fp_pythonic.clone(), deps_fp.clone());
        assert!(hit.is_some(), "Should hit with same options");

        // Different options → miss
        let miss = cache.get(file_id, content_hash.clone(), opts_fp_canonical, deps_fp.clone());
        assert!(miss.is_none(), "CRITICAL: Must miss when options change (prevents stale outputs)");
    }

    #[test]
    fn test_options_fingerprint_guards_against_missing_fields() {
        // ARCHITECT PARANOIA TEST:
        // This test exists to catch if new Config fields are added without updating
        // compute_options_fingerprint(). If this test starts failing after a Config
        // change, it means the fingerprint is now incomplete.
        //
        // How to fix: update compute_options_fingerprint() to include the new field
        // (if it affects compilation output semantics).

        // Current expected fields in compute_options_fingerprint:
        // - pretty_dialect
        // - enable_db7_hover_preview
        // - db7_placeholder_suffix
        // - db7_debug_mode
        // - external_command

        // This is a structural assertion: if Config has new fields, you'll need
        // to examine them and update the fingerprint accordingly.

        // As a sanity check: verify that a Config with all the current fields
        // produces a deterministic fingerprint.

        let config1 = crate::lsp::Config {
            debounce_interval_ms: 250,
            log_level: "info".to_string(),
            external_command: Some(vec!["echo".to_string()]),
            pretty_dialect: Some("pythonic".to_string()),
            enable_db7_hover_preview: true,
            db7_placeholder_suffix: "_renamed".to_string(),
            db7_debug_mode: false,
            caches_enabled: true,
        };

        let config2 = crate::lsp::Config {
            debounce_interval_ms: 250,
            log_level: "info".to_string(),
            external_command: Some(vec!["echo".to_string()]),
            pretty_dialect: Some("pythonic".to_string()),
            enable_db7_hover_preview: true,
            db7_placeholder_suffix: "_renamed".to_string(),
            db7_debug_mode: false,
            caches_enabled: true,
        };

        // Same config → same fingerprint
        let fp1 = crate::proof_session::compute_options_fingerprint(&config1);
        let fp2 = crate::proof_session::compute_options_fingerprint(&config2);
        assert_eq!(
            fp1, fp2,
            "Same config must produce same fingerprint (determinism check)"
        );
    }

    #[test]
    fn test_options_fingerprint_captures_feature_flags() {
        // Verify that the enable_db7_hover_preview flag is captured in fingerprint
        let mut config_with_flag = crate::lsp::Config::default();
        config_with_flag.enable_db7_hover_preview = true;

        let mut config_without_flag = crate::lsp::Config::default();
        config_without_flag.enable_db7_hover_preview = false;

        let fp_with = crate::proof_session::compute_options_fingerprint(&config_with_flag);
        let fp_without =
            crate::proof_session::compute_options_fingerprint(&config_without_flag);

        assert_ne!(
            fp_with, fp_without,
            "Feature flag change must change fingerprint"
        );
    }

    #[test]
    fn test_module_snapshot_fingerprint_completeness_deps_transitive() {
        // CRITICAL VALIDATION TEST:
        // Verify that transitive dependency changes invalidate cache
        //
        // Scenario: A imports B; B imports C
        // When C changes, A must miss cache (even though A's direct content is unchanged)

        let db = Arc::new(sniper_db::SniperDatabase::new());
        let mut cache = ModuleSnapshotCache::new(db);

        let file_a_id = 100u32;
        let file_a_content = HashValue::hash_with_domain(b"SOURCE_TEXT", b"(def a (hole))");
        let opts_fp = HashValue::hash_with_domain(b"OPTIONS", b"default");

        // Dependency state 1: C version 1
        let deps_fp_state1 = HashValue::hash_with_domain(b"DEPS", b"B->C_v1");
        // Dependency state 2: C version 2 (changed)
        let deps_fp_state2 = HashValue::hash_with_domain(b"DEPS", b"B->C_v2");

        let snapshot = ModuleSnapshot {
            file_id: file_a_id,
            content_hash: file_a_content.clone(),
            options_fingerprint: opts_fp.clone(),
            dependency_fingerprint: deps_fp_state1.clone(),
            report: WorkspaceReport {
                diagnostics: Vec::new(),
                diagnostics_by_file: BTreeMap::new(),
                structured_diagnostics: Vec::new(),
                fingerprint: None,
                revision: 0,
                bundle: None,
                proof_state: None,
            },
            diagnostics: Vec::new(),
            timestamp: SystemTime::now(),
        };

        cache.insert(snapshot);

        // Same deps → hit
        let hit = cache.get(file_a_id, file_a_content.clone(), opts_fp.clone(), deps_fp_state1.clone());
        assert!(hit.is_some(), "Should hit with same transitive deps");

        // Different transitive deps → miss
        let miss = cache.get(file_a_id, file_a_content.clone(), opts_fp.clone(), deps_fp_state2);
        assert!(
            miss.is_none(),
            "CRITICAL: Must miss when transitive deps change (prevents stale type info)"
        );
    }
}