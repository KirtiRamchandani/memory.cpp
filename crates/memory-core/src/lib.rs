mod api;
mod compression;
mod config;
mod embedding;
mod engine;
mod error;
mod eval;
mod graph;
mod import;
mod map;
mod ranker;
mod storage;
mod types;
mod vector;

pub use api::{
    askMemory, attachProvider, auditProviderCache, calculateSignalDensity, compileContext,
    compressToolTrace, createContextPack, doctor, estimateKvPressure, estimatePrefill,
    generateBatchPlan, generateRuntimePlan, planProviderCache, recordMemory, recordMistake,
    rollupTrace, scoreAgentReadiness, testMemory, BatchGroup, BatchPlanOptions, BatchPlanReport,
    CacheAuditReport, ContextControlOptions, ContextControlReport, KvPressureReport,
    MemoryRecordOptions, PrefillReport, RuntimePlanOptions, RuntimePlanReport, SignalDensityReport,
    TokenFirewallReport,
};
pub use compression::{CompressionConfig, Compressor, HeuristicCompressor};
pub use config::EngineConfig;
pub use embedding::{Embedder, FastEmbedOnnxEmbedder, HashEmbedder, SharedEmbedder};
pub use engine::MemoryEngine;
pub use error::{MemoryError, Result};
pub use eval::{evaluate, EvalCase, EvalReport, EvalResult};
pub use graph::{extract_entities, Entity, EntityGraph, EntityKind, EntityLink};
pub use import::{
    check_ignored_path, collect_importable_files, import_path, parse_file, ImportFormat,
    ImportOptions, ImportReport, DEFAULT_MEMORYIGNORE,
};
pub use map::{
    MapCitation, MapDiff, MapEdge, MapEdgeKind, MapNode, MapNodeChange, MapNodeClass,
    MapOutputFormat, MapRequest, MapType, MemoryMap,
};
pub use ranker::{RankFeatures, Ranker, RankerConfig};
pub use types::{
    attributes_from_metadata, derive_memory_scores, metadata_for_storage, ConflictRecord,
    EntityStat, ExplainTrace, InboxEntry, MemoryAttributes, MemoryDerivedScores, MemoryDiff,
    MemoryDiffEntry, MemoryEdit, MemoryKind, MemoryLayer, MemoryPermission, MemoryRelation,
    MemorySource, MemoryStats, MemoryStatus, MemorySystemMeta, MemoryVersion, NewMemory,
    PatchResult, PersonaProfile, PolicyMode, PromptContext, RecallQuery, RelationKind, ReplayStep,
    RetentionPolicy, RetrievedMemory, SleepReport, SnapshotRecord, StoredMemory, TimelineEvent,
    WorkspaceInfo,
};

#[cfg(feature = "http")]
pub use embedding::{OllamaEmbedder, OpenAiCompatibleEmbedder};
