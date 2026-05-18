use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};

use chrono::{Duration, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    derive_memory_scores, extract_entities, graph::summarize_links, storage::SqliteStore,
    CompressionConfig, Compressor, ConflictRecord, EngineConfig, ExplainTrace, InboxEntry,
    MemoryDiff, MemoryDiffEntry, MemoryEdit, MemoryError, MemoryKind, MemoryLayer,
    MemoryPermission, MemoryRelation, MemorySource, MemoryStatus, MemoryVersion, NewMemory,
    PatchResult, PersonaProfile, PolicyMode, PromptContext, RankFeatures, Ranker, RecallQuery,
    ReplayStep, Result, RetentionPolicy, RetrievedMemory, SharedEmbedder, SleepReport,
    SnapshotRecord, StoredMemory, TimelineEvent, WorkspaceInfo,
};

#[derive(Clone)]
pub struct MemoryEngine {
    store: Arc<SqliteStore>,
    embedder: SharedEmbedder,
    compressor: crate::HeuristicCompressor,
    ranker: Ranker,
    config: EngineConfig,
}

impl MemoryEngine {
    pub fn open_default(path: impl AsRef<Path>) -> Result<Self> {
        let config = EngineConfig::default();
        let embedder: SharedEmbedder = Arc::new(crate::HashEmbedder::new(config.embedding_dim));
        Self::open_with_config(path, embedder, config)
    }

    pub fn open_with_embedder(path: impl AsRef<Path>, embedder: SharedEmbedder) -> Result<Self> {
        Self::open_with_config(path, embedder, EngineConfig::default())
    }

    pub fn open_with_config(
        path: impl AsRef<Path>,
        embedder: SharedEmbedder,
        config: EngineConfig,
    ) -> Result<Self> {
        let store = Arc::new(SqliteStore::open(path)?);
        let compressor = crate::HeuristicCompressor::new(config.compression.clone());
        let ranker = Ranker::new(config.ranker.clone());

        Ok(Self {
            store,
            embedder,
            compressor,
            ranker,
            config,
        })
    }

    pub fn store_path(&self) -> &Path {
        self.store.path()
    }

    pub fn embedder_name(&self) -> &'static str {
        self.embedder.name()
    }

    pub fn remember(&self, input: NewMemory) -> Result<StoredMemory> {
        let mut input = input;
        if let Some(reason) = detect_sensitive_data(&input.content) {
            return Err(MemoryError::SensitiveData(reason));
        }
        if let Some(policy) = self.resolve_policy(&input.scope, input.kind)? {
            if matches!(policy.mode, PolicyMode::Reject | PolicyMode::NeverStore) {
                return Err(MemoryError::InvalidInput(format!(
                    "policy for scope '{}' forbids storing {} memories",
                    input.scope,
                    input.kind.as_str()
                )));
            }
            apply_policy_to_memory(&mut input, &policy);
        }

        let memory = self.build_memory(input)?;
        self.persist_memory(&memory)?;
        self.record_version(&memory, "create")?;
        self.store.record_event(
            &memory.scope,
            Some(&memory.id),
            "remember",
            "stored memory",
            &json!({
                "memory_id": memory.id,
                "kind": memory.kind,
                "importance": memory.importance,
                "confidence": memory.attributes.confidence
            }),
        )?;
        Ok(memory)
    }

    pub fn remember_candidate(
        &self,
        input: NewMemory,
        reason: &str,
    ) -> Result<Option<StoredMemory>> {
        let mut input = input;
        let policy = self.resolve_policy(&input.scope, input.kind)?;
        if let Some(policy) = &policy {
            apply_policy_to_memory(&mut input, policy);
            if matches!(policy.mode, PolicyMode::Reject | PolicyMode::NeverStore) {
                self.store.record_event(
                    &input.scope,
                    None,
                    "policy_reject",
                    "rejected candidate memory by policy",
                    &json!({ "kind": input.kind, "reason": reason }),
                )?;
                return Ok(None);
            }
        }

        let auto_approve_floor = policy
            .as_ref()
            .and_then(|policy| {
                policy
                    .metadata
                    .get("auto_approve_min_confidence")
                    .and_then(Value::as_f64)
            })
            .unwrap_or(0.55) as f32;

        if detect_sensitive_data(&input.content).is_some()
            || input.attributes.confidence < auto_approve_floor
            || matches!(input.attributes.status, MemoryStatus::PendingReview)
            || matches!(
                policy.as_ref().map(|value| value.mode),
                Some(PolicyMode::ManualReview)
            )
        {
            let inbox = InboxEntry {
                id: Uuid::new_v4().to_string(),
                scope: input.scope.clone(),
                content: input.content,
                reason: reason.to_string(),
                metadata: json!({
                    "memory_cpp": {
                        "candidate_kind": input.kind,
                        "confidence": input.attributes.confidence,
                        "policy_mode": policy.as_ref().map(|value| format!("{:?}", value.mode)),
                    }
                }),
                status: "pending".to_string(),
                created_at: Utc::now(),
            };
            self.store.queue_inbox(&inbox)?;
            self.store.record_event(
                &inbox.scope,
                None,
                "inbox",
                "queued candidate memory for review",
                &json!({ "inbox_id": inbox.id, "reason": inbox.reason }),
            )?;
            return Ok(None);
        }
        self.remember(input).map(Some)
    }

    pub fn search(&self, query: RecallQuery) -> Result<Vec<RetrievedMemory>> {
        self.recall(query)
    }

    pub fn recall(&self, query: RecallQuery) -> Result<Vec<RetrievedMemory>> {
        let started = Instant::now();
        let text = query.text.trim().to_string();
        if text.is_empty() {
            return Err(MemoryError::InvalidInput(
                "recall query is empty".to_string(),
            ));
        }

        let mut query = query;
        query.text = text;
        query.limit = query.limit.max(1);

        let query_embedding = self.embedder.embed(&query.text)?;
        let pool_size = query
            .candidate_pool
            .unwrap_or(self.config.max_candidate_pool)
            .max(query.limit);

        let mut retrieved = self
            .store
            .search(&query_embedding, &query, pool_size)?
            .into_iter()
            .map(|candidate| {
                let (score, reason) = self.ranker.score(RankFeatures {
                    similarity: candidate.similarity,
                    keyword_score: candidate.keyword_score,
                    entity_score: candidate.entity_score,
                    importance: candidate.memory.importance,
                    confidence: candidate.confidence_score,
                    created_at: candidate.memory.created_at,
                    access_count: candidate.memory.access_count,
                    is_sensitive: is_sensitive_memory(&candidate.memory),
                });

                RetrievedMemory {
                    memory: candidate.memory,
                    score,
                    similarity: candidate.similarity,
                    keyword_score: candidate.keyword_score,
                    entity_score: candidate.entity_score,
                    confidence_score: candidate.confidence_score,
                    reason,
                }
            })
            .filter(|memory| memory.score >= query.min_score)
            .collect::<Vec<_>>();

        retrieved.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        retrieved.truncate(query.limit);

        let ids = retrieved
            .iter()
            .map(|item| item.memory.id.clone())
            .collect::<Vec<_>>();
        self.store.mark_accessed(&ids)?;

        let scope = query.scope.clone().unwrap_or_else(|| "default".to_string());
        let latency_ms = started.elapsed().as_millis() as u64;
        self.store.record_event(
            &scope,
            None,
            "recall",
            &query.text,
            &json!({
                "query": query.text,
                "latency_ms": latency_ms,
                "memory_ids": ids,
                "memories": retrieved,
            }),
        )?;

        Ok(retrieved)
    }

    pub fn explain(&self, query: RecallQuery) -> Result<Vec<RetrievedMemory>> {
        self.recall(query)
    }

    pub fn last_explain(&self, scope: Option<&str>) -> Result<Option<ExplainTrace>> {
        let event = self.store.latest_event("recall", scope)?;
        event
            .map(|event| {
                let query = event
                    .data
                    .get("query")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let memories = event
                    .data
                    .get("memories")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();
                Ok(ExplainTrace {
                    query,
                    retrieved_at: event.created_at,
                    memories,
                })
            })
            .transpose()
    }

    pub fn context(&self, query: RecallQuery, token_budget: usize) -> Result<PromptContext> {
        let token_budget = token_budget.max(64);
        let memories = self.recall(query.clone())?;
        let text = build_context_text(&query.text, &memories, token_budget, query.include_content);

        Ok(PromptContext {
            query: query.text,
            token_budget,
            text,
            memories,
        })
    }

    pub fn compact_scope(&self, scope: &str, limit: usize) -> Result<StoredMemory> {
        let scope = normalized_scope(scope)?;
        let memories = self.store.list(Some(&scope), limit.max(1), false)?;
        if memories.is_empty() {
            return Err(MemoryError::InvalidInput(format!(
                "scope '{scope}' has no memories to compact"
            )));
        }

        let content = self
            .compressor
            .compress_collection(&memories, self.config.compaction_max_chars);

        let summary = self.remember(
            NewMemory::new(content)
                .scope(scope.clone())
                .kind(MemoryKind::Summary.as_str())
                .importance(0.8)
                .layer(MemoryLayer::Archival)
                .metadata(json!({
                    "memory_cpp": {
                        "compaction": true,
                        "source_count": memories.len()
                    }
                })),
        )?;

        self.store.record_event(
            &scope,
            Some(&summary.id),
            "compact",
            "compacted workspace memory",
            &json!({ "summary_memory_id": summary.id, "source_count": memories.len() }),
        )?;

        Ok(summary)
    }

    pub fn sleep(&self, scope: &str) -> Result<SleepReport> {
        let scope = normalized_scope(scope)?;
        let memories = self.store.all_memories(Some(&scope), true)?;
        let mut duplicates_superseded = 0;
        let mut conflicts_detected = 0;
        let mut stale_memories_decayed = 0;

        let mut seen = HashMap::<String, String>::new();
        for memory in memories.clone() {
            if !matches!(
                memory.attributes.status,
                MemoryStatus::Active | MemoryStatus::Archived
            ) {
                continue;
            }

            let key = normalize_dedupe_key(&memory.summary, &memory.content);
            if let Some(previous_id) = seen.get(&key) {
                if memory.id != *previous_id {
                    let _ = self.update_status(&memory.id, MemoryStatus::Superseded, "duplicate");
                    duplicates_superseded += 1;
                }
            } else {
                seen.insert(key, memory.id.clone());
            }
        }

        for window in memories.windows(2) {
            let left = &window[0];
            let right = &window[1];
            if looks_like_conflict(left, right) {
                conflicts_detected += 1;
                let conflict = ConflictRecord {
                    id: Uuid::new_v4().to_string(),
                    scope: scope.clone(),
                    old_memory_id: left.id.clone(),
                    new_memory_id: right.id.clone(),
                    status: "open".to_string(),
                    reason: "potential contradictory project fact or preference".to_string(),
                    created_at: Utc::now(),
                };
                self.store.record_conflict(&conflict)?;
            }
        }

        for mut memory in memories {
            if should_decay(&memory) {
                memory.importance = (memory.importance * 0.85).clamp(0.0, 1.0);
                self.persist_memory(&memory)?;
                stale_memories_decayed += 1;
            }
        }

        let summary = self.compact_scope(&scope, 250).ok();
        let report = SleepReport {
            workspace: scope.clone(),
            duplicates_superseded,
            conflicts_detected,
            stale_memories_decayed,
            summary_memory_id: summary.as_ref().map(|memory| memory.id.clone()),
        };

        self.store.record_event(
            &scope,
            report.summary_memory_id.as_deref(),
            "sleep",
            "performed memory consolidation",
            &serde_json::to_value(&report)?,
        )?;

        Ok(report)
    }

    pub fn list_recent(&self, scope: Option<&str>, limit: usize) -> Result<Vec<StoredMemory>> {
        self.store.list(scope, limit, true)
    }

    pub fn all_memories(
        &self,
        scope: Option<&str>,
        include_inactive: bool,
    ) -> Result<Vec<StoredMemory>> {
        self.store.all_memories(scope, include_inactive)
    }

    pub fn entity_graph(&self, scope: Option<&str>, limit: usize) -> Result<crate::EntityGraph> {
        let links = self.store.entity_links(scope, None, limit)?;
        Ok(summarize_links(scope.map(|value| value.to_string()), links))
    }

    pub fn related_entity(
        &self,
        entity: &str,
        scope: Option<&str>,
        limit: usize,
    ) -> Result<Vec<crate::EntityLink>> {
        self.store.entity_links(scope, Some(entity), limit)
    }

    pub fn timeline(
        &self,
        scope: Option<&str>,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TimelineEvent>> {
        self.store.timeline(scope, query, limit)
    }

    pub fn replay(
        &self,
        query: &str,
        scope: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ReplayStep>> {
        let events = self.timeline(scope, Some(query), limit)?;
        let memories = self.recall(
            RecallQuery::new(query)
                .limit(limit)
                .include_inactive(true)
                .workspace(scope.unwrap_or("default").to_string()),
        )?;

        let mut steps = Vec::new();
        for (index, event) in events.into_iter().enumerate() {
            steps.push(ReplayStep {
                index: index + 1,
                event: event.event_type,
                detail: event.body,
                memory_id: event.memory_id,
            });
        }

        for memory in memories {
            let next = steps.len() + 1;
            steps.push(ReplayStep {
                index: next,
                event: "memory".to_string(),
                detail: memory.memory.summary,
                memory_id: Some(memory.memory.id),
            });
        }

        Ok(steps)
    }

    pub fn patch(&self, id: &str, replacement: NewMemory) -> Result<PatchResult> {
        let mut old_memory =
            self.update_status(id, MemoryStatus::Superseded, "memory superseded by patch")?;
        old_memory.attributes.contradiction_count += 1;
        old_memory.updated_at = Utc::now();
        old_memory.derived = derive_memory_scores(
            old_memory.kind,
            &old_memory.content,
            old_memory.importance,
            old_memory.created_at,
            old_memory.updated_at,
            old_memory.access_count,
            &old_memory.attributes,
        );
        self.persist_memory(&old_memory)?;
        self.record_version(&old_memory, "patch_superseded")?;

        let mut replacement = replacement;
        if replacement.scope == "default" {
            replacement.scope = old_memory.scope.clone();
        }
        let new_memory = self.remember(replacement)?;
        self.store.add_relation(
            &old_memory.id,
            &new_memory.id,
            "changed_to",
            1.0,
            &json!({ "reason": "memory patch" }),
        )?;
        self.store.record_event(
            &new_memory.scope,
            Some(&new_memory.id),
            "patch",
            "patched memory with replacement",
            &json!({ "old_memory_id": old_memory.id, "new_memory_id": new_memory.id }),
        )?;

        Ok(PatchResult {
            old_memory,
            new_memory,
            relation: "changed_to".to_string(),
        })
    }

    pub fn update_status(
        &self,
        id: &str,
        status: MemoryStatus,
        reason: &str,
    ) -> Result<StoredMemory> {
        let mut memory = self
            .store
            .get(id)?
            .ok_or_else(|| MemoryError::NotFound(format!("memory '{id}'")))?;
        memory.attributes.status = status;
        memory.updated_at = Utc::now();
        memory.derived = derive_memory_scores(
            memory.kind,
            &memory.content,
            memory.importance,
            memory.created_at,
            memory.updated_at,
            memory.access_count,
            &memory.attributes,
        );
        self.persist_memory(&memory)?;
        self.record_version(&memory, "status_change")?;
        self.store.record_event(
            &memory.scope,
            Some(&memory.id),
            "status",
            reason,
            &json!({ "status": status.as_str() }),
        )?;
        Ok(memory)
    }

    pub fn forget(&self, id: &str, reason: &str) -> Result<StoredMemory> {
        self.update_status(id, MemoryStatus::Forgotten, reason)
    }

    pub fn edit_memory(&self, id: &str, edit: MemoryEdit) -> Result<StoredMemory> {
        let mut memory = self
            .store
            .get(id)?
            .ok_or_else(|| MemoryError::NotFound(format!("memory '{id}'")))?;

        if let Some(content) = edit.content {
            if content.trim().is_empty() {
                return Err(MemoryError::InvalidInput(
                    "edited memory content is empty".to_string(),
                ));
            }
            memory.content = content.trim().to_string();
            memory.summary = self.compressor.compress(&memory.content);
        }
        if let Some(kind) = edit.kind {
            memory.kind = kind;
        }
        if let Some(importance) = edit.importance {
            memory.importance = importance.clamp(0.0, 1.0);
        }
        if let Some(confidence) = edit.confidence {
            memory.attributes.confidence = confidence.clamp(0.0, 1.0);
        }
        if let Some(tags) = edit.tags {
            memory.attributes.tags = tags;
        }
        if let Some(metadata) = edit.metadata {
            memory.metadata = metadata;
        }
        if let Some(source) = edit.source {
            memory.attributes.source = Some(source);
        }
        if let Some(status) = edit.status {
            memory.attributes.status = status;
        }
        memory.updated_at = Utc::now();
        memory.derived = derive_memory_scores(
            memory.kind,
            &memory.content,
            memory.importance,
            memory.created_at,
            memory.updated_at,
            memory.access_count,
            &memory.attributes,
        );

        self.persist_memory(&memory)?;
        self.record_version(&memory, "edit")?;
        self.store.record_event(
            &memory.scope,
            Some(&memory.id),
            "edit",
            "edited memory in place",
            &json!({ "memory_id": memory.id }),
        )?;
        Ok(memory)
    }

    pub fn restore_memory(&self, id: &str) -> Result<StoredMemory> {
        let current = self
            .store
            .get(id)?
            .ok_or_else(|| MemoryError::NotFound(format!("memory '{id}'")))?;
        let versions = self.store.list_versions(id, 32)?;
        let mut restored = if versions.len() > 1 {
            versions[1].snapshot.clone()
        } else {
            current.clone()
        };
        restored.attributes.status = MemoryStatus::Active;
        restored.updated_at = Utc::now();
        restored.derived = derive_memory_scores(
            restored.kind,
            &restored.content,
            restored.importance,
            restored.created_at,
            restored.updated_at,
            restored.access_count,
            &restored.attributes,
        );
        self.persist_memory(&restored)?;
        self.record_version(&restored, "restore")?;
        self.store.record_event(
            &restored.scope,
            Some(&restored.id),
            "restore",
            "restored memory",
            &json!({ "memory_id": restored.id }),
        )?;
        Ok(restored)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        self.store.delete(id)
    }

    pub fn create_workspace(
        &self,
        name: &str,
        description: &str,
        category: &str,
        active: bool,
    ) -> Result<WorkspaceInfo> {
        let workspace = WorkspaceInfo {
            name: normalized_scope(name)?,
            description: description.to_string(),
            category: category.to_string(),
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            active,
        };
        self.store.upsert_workspace(&workspace)?;
        self.store.record_event(
            &workspace.name,
            None,
            "workspace",
            "created workspace",
            &serde_json::to_value(&workspace)?,
        )?;
        Ok(workspace)
    }

    pub fn switch_workspace(&self, name: &str) -> Result<WorkspaceInfo> {
        let existing = self
            .store
            .list_workspaces()?
            .into_iter()
            .find(|workspace| workspace.name == name)
            .ok_or_else(|| MemoryError::NotFound(format!("workspace '{name}'")))?;

        let workspace = WorkspaceInfo {
            active: true,
            updated_at: Utc::now(),
            ..existing
        };
        self.store.upsert_workspace(&workspace)?;
        Ok(workspace)
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>> {
        self.store.list_workspaces()
    }

    pub fn current_workspace(&self) -> Result<Option<WorkspaceInfo>> {
        self.store.current_workspace()
    }

    pub fn set_policy(
        &self,
        scope: &str,
        memory_type: Option<String>,
        mode: PolicyMode,
        retain_days: Option<u32>,
        metadata: serde_json::Value,
    ) -> Result<RetentionPolicy> {
        let policy = RetentionPolicy {
            id: Uuid::new_v4().to_string(),
            scope: normalized_scope(scope)?,
            memory_type,
            mode,
            retain_days,
            metadata,
            created_at: Utc::now(),
        };
        self.store.upsert_policy(&policy)?;
        Ok(policy)
    }

    pub fn list_policies(&self, scope: Option<&str>) -> Result<Vec<RetentionPolicy>> {
        self.store.list_policies(scope)
    }

    pub fn save_snapshot(&self, scope: &str, name: &str) -> Result<SnapshotRecord> {
        let scope = normalized_scope(scope)?;
        let snapshot = SnapshotRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            scope: scope.clone(),
            memories: self.store.all_memories(Some(&scope), true)?,
            created_at: Utc::now(),
        };
        self.store.save_snapshot(&snapshot)?;
        self.store.record_event(
            &scope,
            None,
            "snapshot",
            "saved memory snapshot",
            &json!({ "snapshot_id": snapshot.id, "name": snapshot.name }),
        )?;
        Ok(snapshot)
    }

    pub fn restore_snapshot(&self, scope: &str, name: &str) -> Result<usize> {
        let snapshot = self
            .store
            .snapshot_by_name(scope, name)?
            .ok_or_else(|| MemoryError::NotFound(format!("snapshot '{name}'")))?;
        for memory in &snapshot.memories {
            self.persist_memory(memory)?;
        }
        self.store.record_event(
            scope,
            None,
            "snapshot_restore",
            "restored snapshot",
            &json!({ "snapshot_id": snapshot.id, "name": snapshot.name }),
        )?;
        Ok(snapshot.memories.len())
    }

    pub fn list_snapshots(&self, scope: Option<&str>, limit: usize) -> Result<Vec<SnapshotRecord>> {
        let mut snapshots = self.store.list_snapshots(scope, limit)?;
        snapshots.truncate(limit);
        Ok(snapshots)
    }

    pub fn snapshot_named(&self, scope: &str, name: &str) -> Result<Option<SnapshotRecord>> {
        self.store.snapshot_by_name(scope, name)
    }

    pub fn diff_snapshots(&self, scope: &str, left: &str, right: &str) -> Result<MemoryDiff> {
        let left_snapshot = self
            .store
            .snapshot_by_name(scope, left)?
            .ok_or_else(|| MemoryError::NotFound(format!("snapshot '{left}'")))?;
        let right_snapshot = self
            .store
            .snapshot_by_name(scope, right)?
            .ok_or_else(|| MemoryError::NotFound(format!("snapshot '{right}'")))?;

        let left_map = left_snapshot
            .memories
            .iter()
            .map(|memory| (memory.id.clone(), memory))
            .collect::<HashMap<_, _>>();
        let right_map = right_snapshot
            .memories
            .iter()
            .map(|memory| (memory.id.clone(), memory))
            .collect::<HashMap<_, _>>();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        for memory in &right_snapshot.memories {
            if let Some(old) = left_map.get(&memory.id) {
                if old.summary != memory.summary || old.content != memory.content {
                    changed.push(MemoryDiffEntry {
                        kind: "changed".to_string(),
                        memory_id: memory.id.clone(),
                        summary: memory.summary.clone(),
                    });
                }
            } else {
                added.push(MemoryDiffEntry {
                    kind: "added".to_string(),
                    memory_id: memory.id.clone(),
                    summary: memory.summary.clone(),
                });
            }
        }

        for memory in &left_snapshot.memories {
            if !right_map.contains_key(&memory.id) {
                removed.push(MemoryDiffEntry {
                    kind: "removed".to_string(),
                    memory_id: memory.id.clone(),
                    summary: memory.summary.clone(),
                });
            }
        }

        Ok(MemoryDiff {
            left: left.to_string(),
            right: right.to_string(),
            added,
            removed,
            changed,
        })
    }

    pub fn export_persona(&self, scope: &str, name: &str) -> Result<PersonaProfile> {
        let memories = self.store.all_memories(Some(scope), false)?;
        let mut preferences = Vec::new();
        let mut workflows = Vec::new();
        let mut rules = Vec::new();

        for memory in memories {
            match memory.kind {
                MemoryKind::Preference | MemoryKind::Persona => preferences.push(memory),
                MemoryKind::Workflow => workflows.push(memory),
                _ if memory.attributes.tags.iter().any(|tag| tag == "rule") => rules.push(memory),
                _ => {}
            }
        }

        Ok(PersonaProfile {
            name: name.to_string(),
            exported_at: Utc::now(),
            preferences,
            workflows,
            rules,
        })
    }

    pub fn import_persona(&self, scope: &str, profile: PersonaProfile) -> Result<usize> {
        let mut imported = 0;
        for memory in profile
            .preferences
            .into_iter()
            .chain(profile.workflows)
            .chain(profile.rules)
        {
            let mut cloned = memory.clone();
            cloned.scope = scope.to_string();
            self.persist_memory(&cloned)?;
            imported += 1;
        }
        Ok(imported)
    }

    pub fn queue_inbox(
        &self,
        scope: &str,
        content: &str,
        reason: &str,
        metadata: Value,
    ) -> Result<InboxEntry> {
        let entry = InboxEntry {
            id: Uuid::new_v4().to_string(),
            scope: scope.to_string(),
            content: content.to_string(),
            reason: reason.to_string(),
            metadata,
            status: "pending".to_string(),
            created_at: Utc::now(),
        };
        self.store.queue_inbox(&entry)?;
        Ok(entry)
    }

    pub fn inbox(&self, scope: Option<&str>, status: Option<&str>) -> Result<Vec<InboxEntry>> {
        self.store.list_inbox(scope, status)
    }

    pub fn list_versions(&self, memory_id: &str, limit: usize) -> Result<Vec<MemoryVersion>> {
        self.store.list_versions(memory_id, limit)
    }

    pub fn relations(&self, scope: Option<&str>, limit: usize) -> Result<Vec<MemoryRelation>> {
        self.store.list_relations(scope, limit)
    }

    pub fn review_inbox(&self, id: &str, action: &str) -> Result<bool> {
        self.store.resolve_inbox(id, action)
    }

    pub fn conflicts(&self, scope: Option<&str>, limit: usize) -> Result<Vec<ConflictRecord>> {
        self.store.list_conflicts(scope, limit)
    }

    pub fn stats(&self) -> Result<crate::MemoryStats> {
        self.store.stats(self.embedder_name())
    }

    pub fn default_compression_config() -> CompressionConfig {
        CompressionConfig::default()
    }

    fn build_memory(&self, input: NewMemory) -> Result<StoredMemory> {
        let content = input.content.trim();
        if content.is_empty() {
            return Err(MemoryError::InvalidInput(
                "memory content is empty".to_string(),
            ));
        }

        let scope = normalized_scope(&input.scope)?;
        let summary = self.compressor.compress(content);
        let now = input.created_at.unwrap_or_else(Utc::now);
        let mut attributes = input.attributes;
        attributes.confidence = attributes
            .confidence
            .clamp(0.0, 1.0)
            .max(estimate_confidence(content, &attributes.source));
        let importance = input
            .importance
            .unwrap_or_else(|| estimate_importance(content, input.kind))
            .clamp(0.0, 1.0);
        let derived =
            derive_memory_scores(input.kind, content, importance, now, now, 0, &attributes);

        Ok(StoredMemory {
            id: Uuid::new_v4().to_string(),
            kind: input.kind,
            scope,
            content: content.to_string(),
            summary,
            metadata: input.metadata,
            importance,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            access_count: 0,
            attributes,
            derived,
        })
    }

    fn persist_memory(&self, memory: &StoredMemory) -> Result<()> {
        let mut memory = memory.clone();
        memory.derived = derive_memory_scores(
            memory.kind,
            &memory.content,
            memory.importance,
            memory.created_at,
            memory.updated_at,
            memory.access_count,
            &memory.attributes,
        );
        let embedding_text = format!("{}\n{}", memory.summary, memory.content);
        let embedding = self.embedder.embed(&embedding_text)?;
        self.store.upsert(&memory, &embedding)?;
        let entity_text = format!("{}\n{}", memory.summary, memory.content);
        let entities = extract_entities(&entity_text);
        self.store.index_entities(&memory, &entities)?;
        Ok(())
    }

    fn record_version(&self, memory: &StoredMemory, action: &str) -> Result<()> {
        let version = MemoryVersion {
            id: Uuid::new_v4().to_string(),
            memory_id: memory.id.clone(),
            action: action.to_string(),
            snapshot: memory.clone(),
            created_at: Utc::now(),
        };
        self.store.record_version(&version)
    }

    fn resolve_policy(&self, scope: &str, kind: MemoryKind) -> Result<Option<RetentionPolicy>> {
        let scope = normalized_scope(scope)?;
        let mut policies = self.store.list_policies(Some(&scope))?;
        let mut global_policies = self.store.list_policies(Some("global"))?;
        policies.append(&mut global_policies);

        if let Some(policy) = policies
            .iter()
            .find(|policy| policy.memory_type.as_deref() == Some(kind.as_str()))
        {
            return Ok(Some(policy.clone()));
        }

        Ok(policies
            .iter()
            .find(|policy| policy.memory_type.is_none())
            .cloned())
    }
}

fn normalized_scope(scope: &str) -> Result<String> {
    let scope = scope.trim();
    if scope.is_empty() {
        return Err(MemoryError::InvalidInput(
            "memory scope is empty".to_string(),
        ));
    }

    Ok(scope.to_string())
}

fn apply_policy_to_memory(memory: &mut NewMemory, policy: &RetentionPolicy) {
    if let Some(retain_days) = policy.retain_days {
        let expires_at = Utc::now() + Duration::days(retain_days as i64);
        if memory.attributes.expires_at.is_none() {
            memory.attributes.expires_at = Some(expires_at);
        }
    }

    match policy.mode {
        PolicyMode::Ephemeral | PolicyMode::SessionOnly => {
            memory.attributes.status = MemoryStatus::Ephemeral;
            memory.attributes.permission = MemoryPermission::Ephemeral;
            if matches!(policy.mode, PolicyMode::SessionOnly) {
                memory.attributes.layer = MemoryLayer::Session;
            }
        }
        PolicyMode::Decay | PolicyMode::Forever | PolicyMode::ManualReview => {}
        PolicyMode::NeverStore | PolicyMode::Reject => {}
    }
}

fn estimate_importance(content: &str, kind: MemoryKind) -> f32 {
    let lower = content.to_ascii_lowercase();
    let mut score: f32 = match kind {
        MemoryKind::Preference => 0.74,
        MemoryKind::Fact => 0.68,
        MemoryKind::Task => 0.64,
        MemoryKind::Code => 0.62,
        MemoryKind::Summary => 0.8,
        MemoryKind::Decision => 0.84,
        MemoryKind::Workflow => 0.76,
        MemoryKind::Bug => 0.73,
        MemoryKind::Persona => 0.86,
        MemoryKind::Event | MemoryKind::Note => 0.52,
    };

    for keyword in [
        "always",
        "never",
        "prefer",
        "important",
        "decision",
        "remember",
        "deadline",
        "api",
        "performance",
        "migrate",
        "stack",
    ] {
        if lower.contains(keyword) {
            score += 0.04;
        }
    }

    if content.len() > 280 {
        score += 0.04;
    }

    score.clamp(0.0, 1.0)
}

fn estimate_confidence(content: &str, source: &Option<MemorySource>) -> f32 {
    let mut confidence: f32 = 0.72;

    if let Some(source) = source {
        if let Some(reliability) = source.reliability {
            confidence = confidence.max(reliability.clamp(0.0, 1.0));
        }
        if source
            .created_by
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("user"))
        {
            confidence += 0.12;
        }
    }

    if content.to_ascii_lowercase().contains("maybe") {
        confidence -= 0.18;
    }

    confidence.clamp(0.0, 1.0)
}

fn build_context_text(
    query: &str,
    memories: &[RetrievedMemory],
    token_budget: usize,
    include_content: bool,
) -> String {
    let char_budget = token_budget.saturating_mul(4);
    let mut output = format!("Relevant long-term memory for query: {query}\n");

    for item in memories {
        let body = if include_content {
            &item.memory.content
        } else {
            &item.memory.summary
        };

        let citation = format!(
            "[{} | {}]",
            item.memory.id,
            item.memory.created_at.format("%Y-%m-%d")
        );
        let line = format!(
            "- {citation} [{:.3} | {} | {}] {}\n",
            item.score, item.memory.kind, item.memory.scope, body
        );

        if output.len() + line.len() > char_budget {
            break;
        }

        output.push_str(&line);
    }

    output.trim_end().to_string()
}

fn detect_sensitive_data(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let patterns = [
        ("api_key", "looks like an API key"),
        ("-----begin", "looks like a private key"),
        ("password", "contains password-like material"),
        ("secret", "contains a secret-like token"),
        ("ssn", "contains sensitive identity information"),
        ("aadhaar", "contains sensitive identity information"),
    ];

    for (needle, reason) in patterns {
        if lower.contains(needle) {
            return Some(reason.to_string());
        }
    }

    if text.contains("sk-") || text.contains("ghp_") {
        return Some("looks like an authentication token".to_string());
    }

    None
}

fn normalize_dedupe_key(summary: &str, content: &str) -> String {
    format!("{summary}\n{content}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn looks_like_conflict(left: &StoredMemory, right: &StoredMemory) -> bool {
    if left.scope != right.scope || left.id == right.id {
        return false;
    }

    if !(matches!(
        left.kind,
        MemoryKind::Fact | MemoryKind::Preference | MemoryKind::Decision
    ) && matches!(
        right.kind,
        MemoryKind::Fact | MemoryKind::Preference | MemoryKind::Decision
    )) {
        return false;
    }

    let left_entities = extract_entities(&left.summary)
        .into_iter()
        .map(|entity| entity.name)
        .collect::<Vec<_>>();
    let right_entities = extract_entities(&right.summary)
        .into_iter()
        .map(|entity| entity.name)
        .collect::<Vec<_>>();

    if left_entities.is_empty() || right_entities.is_empty() {
        return false;
    }

    let overlap = left_entities
        .iter()
        .filter(|entity| right_entities.iter().any(|value| value == *entity))
        .count();

    overlap > 0 && left.summary != right.summary
}

fn should_decay(memory: &StoredMemory) -> bool {
    matches!(
        memory.attributes.status,
        MemoryStatus::Active | MemoryStatus::Archived
    ) && (Utc::now() - memory.updated_at).num_days() > 30
        && memory.access_count == 0
}

fn is_sensitive_memory(memory: &StoredMemory) -> bool {
    detect_sensitive_data(&memory.content).is_some()
        || memory.attributes.permission == MemoryPermission::Encrypted
}
