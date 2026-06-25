mod api;
mod autopsy;
mod bench;
mod cache_manifest;
mod compression;
mod config;
mod contradictions;
mod debt;
mod embedding;
mod engine;
mod error;
mod eval;
mod freshness;
mod fuzz;
mod gc;
mod graph;
mod import;
mod ir;
mod kv_model;
mod ledger;
mod lockfile;
mod map;
mod minimize;
mod negative;
mod optimizer;
mod poison;
mod prefix;
mod ranker;
mod rehydrate;
mod storage;
mod tokenizer;
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
pub use autopsy::{analyze as autopsy_analyze, Autopsy, AutopsyInput, Finding};
pub use bench::{run as run_camc_bench, BenchReport, Strategy, StrategyResult};
pub use cache_manifest::{compiler_version, CacheManifest, CacheManifestInput, CacheRegion};
pub use compression::{CompressionConfig, Compressor, HeuristicCompressor};
pub use config::EngineConfig;
pub use contradictions::{detect as detect_contradictions, Contradiction, MemoryView, MergePolicy};
pub use debt::{score as debt_score, DebtFactors, DebtReport};
pub use embedding::{Embedder, FastEmbedOnnxEmbedder, HashEmbedder, SharedEmbedder};
pub use engine::MemoryEngine;
pub use error::{MemoryError, Result};
pub use eval::{evaluate, EvalCase, EvalReport, EvalResult};
pub use freshness::{
    prove as prove_freshness, FreshnessInput, FreshnessProof, FreshnessVerdict, ProofCheck,
    ProofStatus,
};
pub use fuzz::{fuzz, FuzzBlock, FuzzReport, MutationOutcome};
pub use gc::{plan as gc_plan, GcAction, GcDecision, GcInput, GcReport};
pub use graph::{extract_entities, Entity, EntityGraph, EntityKind, EntityLink};
pub use import::{
    check_ignored_path, collect_importable_files, import_path, parse_file, ImportFormat,
    ImportOptions, ImportReport, DEFAULT_MEMORYIGNORE,
};
pub use ir::{CacheRole, IrBlock, IrKind, MemoryIr, Volatility};
pub use kv_model::{mib, KvComparison, ModelProfile};
pub use ledger::{ExcludedRecord, LedgerEntry, PackChecksums};
pub use lockfile::{ContextPackLock, LockTokenCounts};
pub use map::{
    MapCitation, MapDiff, MapEdge, MapEdgeKind, MapNode, MapNodeChange, MapNodeClass,
    MapOutputFormat, MapRequest, MapType, MemoryMap,
};
pub use minimize::{minimize, MinBlock, MinStrategy, MinimizeResult};
pub use negative::{NegativeKind, NegativeMemory, NegativeMemoryStore};
pub use optimizer::{
    optimize, Objective, OptLevel, OptimizationResult, OptimizerConfig, ScoredBlock,
};
pub use poison::{
    benchmark as poison_benchmark, screen_write, FirewallVerdict, ScenarioOutcome, WriteCandidate,
};
pub use prefix::{
    canonicalize_stable_prefix, content_hash, pack_id, render_fresh_suffix, stable_prefix_hash,
    FreshSuffixInput, StablePrefixInput, PACK_VERSION, STABLE_PREFIX_VERSION,
};
pub use ranker::{RankFeatures, Ranker, RankerConfig};
pub use rehydrate::{rehydrate_file, Handle, Rehydrated};
pub use tokenizer::{estimate_tokens, TokenCount, Tokenizer, TokenizerMode};
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
