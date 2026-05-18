use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{MemoryError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Event,
    Task,
    Code,
    Note,
    Summary,
    Decision,
    Bug,
    Workflow,
    Persona,
}

impl MemoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Event => "event",
            Self::Task => "task",
            Self::Code => "code",
            Self::Note => "note",
            Self::Summary => "summary",
            Self::Decision => "decision",
            Self::Bug => "bug",
            Self::Workflow => "workflow",
            Self::Persona => "persona",
        }
    }
}

impl fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MemoryKind {
    type Err = MemoryError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fact" => Ok(Self::Fact),
            "preference" | "pref" => Ok(Self::Preference),
            "event" => Ok(Self::Event),
            "task" | "todo" => Ok(Self::Task),
            "code" => Ok(Self::Code),
            "note" => Ok(Self::Note),
            "summary" | "compact" | "compaction" => Ok(Self::Summary),
            "decision" => Ok(Self::Decision),
            "bug" | "issue" => Ok(Self::Bug),
            "workflow" | "procedure" => Ok(Self::Workflow),
            "persona" | "identity" => Ok(Self::Persona),
            other => Err(MemoryError::InvalidKind(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    #[default]
    Active,
    Archived,
    Superseded,
    Contradicted,
    Forgotten,
    Ephemeral,
    PendingReview,
}

impl MemoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Superseded => "superseded",
            Self::Contradicted => "contradicted",
            Self::Forgotten => "forgotten",
            Self::Ephemeral => "ephemeral",
            Self::PendingReview => "pending_review",
        }
    }
    pub fn is_recallable(self) -> bool {
        matches!(self, Self::Active | Self::Archived | Self::Ephemeral)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPermission {
    #[default]
    Private,
    WorkspaceOnly,
    AgentSpecific,
    Shareable,
    Encrypted,
    Ephemeral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayer {
    Working,
    Session,
    Episodic,
    #[default]
    Semantic,
    Procedural,
    Identity,
    Project,
    Archival,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    Forever,
    Decay,
    Ephemeral,
    SessionOnly,
    ManualReview,
    NeverStore,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Uses,
    Prefers,
    WorksOn,
    DependsOn,
    ChangedTo,
    Contradicts,
    Caused,
    FixedBy,
    MentionedIn,
    Supersedes,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySource {
    pub source_type: Option<String>,
    pub source_app: Option<String>,
    pub source: Option<String>,
    pub source_file: Option<String>,
    pub source_line: Option<u64>,
    pub source_commit: Option<String>,
    pub source_conversation_id: Option<String>,
    pub source_message_id: Option<String>,
    pub created_by: Option<String>,
    pub reliability: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDerivedScores {
    pub freshness: f32,
    pub usefulness: f32,
    pub trust: f32,
    pub sensitivity: f32,
    pub source_reliability: f32,
    pub explanation: Vec<String>,
}

impl Default for MemoryDerivedScores {
    fn default() -> Self {
        Self {
            freshness: 0.5,
            usefulness: 0.5,
            trust: 0.5,
            sensitivity: 0.0,
            source_reliability: 0.5,
            explanation: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAttributes {
    pub tags: Vec<String>,
    pub confidence: f32,
    pub source: Option<MemorySource>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: MemoryStatus,
    pub permission: MemoryPermission,
    pub layer: MemoryLayer,
    pub human_confirmed: bool,
    pub contradiction_count: u32,
}

impl Default for MemoryAttributes {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            confidence: 0.8,
            source: None,
            last_verified_at: None,
            expires_at: None,
            status: MemoryStatus::Active,
            permission: MemoryPermission::Private,
            layer: MemoryLayer::Semantic,
            human_confirmed: false,
            contradiction_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMemory {
    pub content: String,
    pub kind: MemoryKind,
    pub scope: String,
    pub metadata: Value,
    pub importance: Option<f32>,
    pub attributes: MemoryAttributes,
    pub created_at: Option<DateTime<Utc>>,
}

impl NewMemory {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            kind: MemoryKind::Note,
            scope: "default".to_string(),
            metadata: Value::Object(Default::default()),
            importance: None,
            attributes: MemoryAttributes::default(),
            created_at: None,
        }
    }

    pub fn kind(mut self, kind: impl AsRef<str>) -> Self {
        self.kind = MemoryKind::from_str(kind.as_ref()).unwrap_or(MemoryKind::Note);
        self
    }

    pub fn try_kind(mut self, kind: impl AsRef<str>) -> Result<Self> {
        self.kind = MemoryKind::from_str(kind.as_ref())?;
        Ok(self)
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = scope.into();
        self
    }

    pub fn workspace(self, workspace: impl Into<String>) -> Self {
        self.scope(workspace)
    }

    pub fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn importance(mut self, importance: f32) -> Self {
        self.importance = Some(importance.clamp(0.0, 1.0));
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.attributes.tags.push(tag.into());
        self
    }

    pub fn tags(mut self, tags: impl IntoIterator<Item = String>) -> Self {
        self.attributes.tags.extend(tags);
        self
    }

    pub fn confidence(mut self, confidence: f32) -> Self {
        self.attributes.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn source(mut self, source: MemorySource) -> Self {
        self.attributes.source = Some(source);
        self
    }

    pub fn status(mut self, status: MemoryStatus) -> Self {
        self.attributes.status = status;
        self
    }

    pub fn permission(mut self, permission: MemoryPermission) -> Self {
        self.attributes.permission = permission;
        self
    }

    pub fn layer(mut self, layer: MemoryLayer) -> Self {
        self.attributes.layer = layer;
        self
    }

    pub fn created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn last_verified_at(mut self, last_verified_at: DateTime<Utc>) -> Self {
        self.attributes.last_verified_at = Some(last_verified_at);
        self
    }

    pub fn expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.attributes.expires_at = Some(expires_at);
        self
    }

    pub fn human_confirmed(mut self, human_confirmed: bool) -> Self {
        self.attributes.human_confirmed = human_confirmed;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMemory {
    pub id: String,
    pub kind: MemoryKind,
    pub scope: String,
    pub content: String,
    pub summary: String,
    pub metadata: Value,
    pub importance: f32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub access_count: u64,
    pub attributes: MemoryAttributes,
    pub derived: MemoryDerivedScores,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedMemory {
    pub memory: StoredMemory,
    pub score: f32,
    pub similarity: f32,
    pub keyword_score: f32,
    pub entity_score: f32,
    pub confidence_score: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallQuery {
    pub text: String,
    pub scope: Option<String>,
    pub kinds: Vec<MemoryKind>,
    pub limit: usize,
    pub min_score: f32,
    pub candidate_pool: Option<usize>,
    pub include_content: bool,
    pub tags: Vec<String>,
    pub include_inactive: bool,
    pub include_global: bool,
}

impl RecallQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            scope: None,
            kinds: Vec::new(),
            limit: 8,
            min_score: 0.0,
            candidate_pool: None,
            include_content: false,
            tags: Vec::new(),
            include_inactive: false,
            include_global: true,
        }
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    pub fn workspace(self, workspace: impl Into<String>) -> Self {
        self.scope(workspace)
    }

    pub fn kind(mut self, kind: MemoryKind) -> Self {
        self.kinds.push(kind);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    pub fn min_score(mut self, min_score: f32) -> Self {
        self.min_score = min_score;
        self
    }

    pub fn candidate_pool(mut self, candidate_pool: usize) -> Self {
        self.candidate_pool = Some(candidate_pool.max(self.limit));
        self
    }

    pub fn include_content(mut self, include_content: bool) -> Self {
        self.include_content = include_content;
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn include_inactive(mut self, include_inactive: bool) -> Self {
        self.include_inactive = include_inactive;
        self
    }

    pub fn include_global(mut self, include_global: bool) -> Self {
        self.include_global = include_global;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptContext {
    pub query: String,
    pub token_budget: usize,
    pub text: String,
    pub memories: Vec<RetrievedMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityStat {
    pub name: String,
    pub kind: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub memories: u64,
    pub workspaces: u64,
    pub bytes: u64,
    pub oldest_memory_at: Option<DateTime<Utc>>,
    pub newest_memory_at: Option<DateTime<Utc>>,
    pub embedding_model: String,
    pub average_recall_latency_ms: f32,
    pub stale_memories: u64,
    pub conflicts: u64,
    pub top_entities: Vec<EntityStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub memory_id: Option<String>,
    pub scope: String,
    pub event_type: String,
    pub body: String,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub id: String,
    pub scope: String,
    pub memory_type: Option<String>,
    pub mode: PolicyMode,
    pub retain_days: Option<u32>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub memories: Vec<StoredMemory>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub id: String,
    pub scope: String,
    pub old_memory_id: String,
    pub new_memory_id: String,
    pub status: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxEntry {
    pub id: String,
    pub scope: String,
    pub content: String,
    pub reason: String,
    pub metadata: Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDiffEntry {
    pub kind: String,
    pub memory_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDiff {
    pub left: String,
    pub right: String,
    pub added: Vec<MemoryDiffEntry>,
    pub removed: Vec<MemoryDiffEntry>,
    pub changed: Vec<MemoryDiffEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayStep {
    pub index: usize,
    pub event: String,
    pub detail: String,
    pub memory_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepReport {
    pub workspace: String,
    pub duplicates_superseded: usize,
    pub conflicts_detected: usize,
    pub stale_memories_decayed: usize,
    pub summary_memory_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchResult {
    pub old_memory: StoredMemory,
    pub new_memory: StoredMemory,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVersion {
    pub id: String,
    pub memory_id: String,
    pub action: String,
    pub snapshot: StoredMemory,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryEdit {
    pub content: Option<String>,
    pub kind: Option<MemoryKind>,
    pub importance: Option<f32>,
    pub confidence: Option<f32>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<Value>,
    pub source: Option<MemorySource>,
    pub status: Option<MemoryStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRelation {
    pub id: String,
    pub source_memory_id: String,
    pub target_memory_id: String,
    pub relation: String,
    pub weight: f32,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaProfile {
    pub name: String,
    pub exported_at: DateTime<Utc>,
    pub preferences: Vec<StoredMemory>,
    pub workflows: Vec<StoredMemory>,
    pub rules: Vec<StoredMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainTrace {
    pub query: String,
    pub retrieved_at: DateTime<Utc>,
    pub memories: Vec<RetrievedMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySystemMeta {
    pub tags: Vec<String>,
    pub confidence: f32,
    pub source: Option<MemorySource>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: Option<MemoryStatus>,
    pub permission: Option<MemoryPermission>,
    pub layer: Option<MemoryLayer>,
    pub human_confirmed: bool,
    pub contradiction_count: u32,
}

pub fn derive_memory_scores(
    kind: MemoryKind,
    content: &str,
    importance: f32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    access_count: u64,
    attributes: &MemoryAttributes,
) -> MemoryDerivedScores {
    let age_days = (Utc::now() - updated_at).num_days().max(0) as f32;
    let freshness = (1.0 - (age_days / 90.0)).clamp(0.05, 1.0);
    let source_reliability = attributes
        .source
        .as_ref()
        .and_then(|value| value.reliability)
        .unwrap_or(0.6)
        .clamp(0.0, 1.0);
    let trust = ((attributes.confidence * 0.65)
        + (source_reliability * 0.25)
        + if attributes.human_confirmed { 0.1 } else { 0.0 })
    .clamp(0.0, 1.0);
    let usage_factor = ((access_count as f32) / 8.0).clamp(0.0, 1.0);
    let usefulness =
        ((importance * 0.45) + (freshness * 0.2) + (trust * 0.2) + (usage_factor * 0.15))
            .clamp(0.0, 1.0);

    let lower = content.to_ascii_lowercase();
    let sensitive_hits = [
        "api_key",
        "password",
        "secret",
        "token",
        "private key",
        "ssn",
        "credit card",
        "medical",
    ]
    .into_iter()
    .filter(|needle| lower.contains(needle))
    .count() as f32;
    let permission_bias = match attributes.permission {
        MemoryPermission::Encrypted => 0.8,
        MemoryPermission::Ephemeral => 0.55,
        MemoryPermission::AgentSpecific => 0.35,
        MemoryPermission::WorkspaceOnly => 0.25,
        MemoryPermission::Private => 0.15,
        MemoryPermission::Shareable => 0.05,
    };
    let sensitivity = (permission_bias + (sensitive_hits * 0.2)).clamp(0.0, 1.0);

    let mut explanation = vec![
        format!("freshness uses updated_at age ({age_days:.0}d old)"),
        format!(
            "trust combines confidence {:.2} and source reliability {:.2}",
            attributes.confidence, source_reliability
        ),
        format!(
            "usefulness combines importance {:.2}, freshness {:.2}, trust {:.2}, and access count {}",
            importance, freshness, trust, access_count
        ),
        format!(
            "sensitivity combines permission {} and content inspection",
            match attributes.permission {
                MemoryPermission::Private => "private",
                MemoryPermission::WorkspaceOnly => "workspace_only",
                MemoryPermission::AgentSpecific => "agent_specific",
                MemoryPermission::Shareable => "shareable",
                MemoryPermission::Encrypted => "encrypted",
                MemoryPermission::Ephemeral => "ephemeral",
            }
        ),
    ];

    if matches!(
        kind,
        MemoryKind::Decision | MemoryKind::Workflow | MemoryKind::Persona
    ) {
        explanation.push(
            "decision/workflow/persona memories receive higher downstream usefulness".to_string(),
        );
    }
    if created_at != updated_at {
        explanation.push(
            "memory has historical edits, so freshness is based on the latest update".to_string(),
        );
    }

    MemoryDerivedScores {
        freshness,
        usefulness,
        trust,
        sensitivity,
        source_reliability,
        explanation,
    }
}

pub fn metadata_for_storage(metadata: &Value, attributes: &MemoryAttributes) -> Value {
    let mut merged = metadata.clone();
    if !merged.is_object() {
        merged = Value::Object(Default::default());
    }

    let system = MemorySystemMeta {
        tags: attributes.tags.clone(),
        confidence: attributes.confidence,
        source: attributes.source.clone(),
        last_verified_at: attributes.last_verified_at,
        expires_at: attributes.expires_at,
        status: Some(attributes.status),
        permission: Some(attributes.permission),
        layer: Some(attributes.layer),
        human_confirmed: attributes.human_confirmed,
        contradiction_count: attributes.contradiction_count,
    };

    if let Some(object) = merged.as_object_mut() {
        object.insert(
            "memory_cpp".to_string(),
            serde_json::to_value(system).unwrap_or_else(|_| Value::Object(Default::default())),
        );
    }

    merged
}

pub fn attributes_from_metadata(metadata: &Value) -> MemoryAttributes {
    let system = metadata
        .get("memory_cpp")
        .cloned()
        .and_then(|value| serde_json::from_value::<MemorySystemMeta>(value).ok())
        .unwrap_or_default();
    let defaults = MemoryAttributes::default();

    MemoryAttributes {
        tags: system.tags,
        confidence: if system.confidence > 0.0 {
            system.confidence.clamp(0.0, 1.0)
        } else {
            defaults.confidence
        },
        source: system.source,
        last_verified_at: system.last_verified_at,
        expires_at: system.expires_at,
        status: system.status.unwrap_or_default(),
        permission: system.permission.unwrap_or_default(),
        layer: system.layer.unwrap_or_default(),
        human_confirmed: system.human_confirmed,
        contradiction_count: system.contradiction_count,
    }
}
