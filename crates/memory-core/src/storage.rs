use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Mutex,
};

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::{
    attributes_from_metadata, derive_memory_scores, extract_entities,
    graph::{Entity, EntityKind, EntityLink},
    metadata_for_storage,
    types::{
        ConflictRecord, EntityStat, InboxEntry, MemoryRelation, MemoryStats, MemoryVersion,
        RecallQuery, RetentionPolicy, SnapshotRecord, StoredMemory, TimelineEvent, WorkspaceInfo,
    },
    vector::{cosine_similarity, deserialize_f32_vec, serialize_f32_vec},
    MemoryError, MemoryKind, Result,
};

const SELECT_FIELDS: &str = "id, kind, scope, content, summary, metadata_json, importance, \
created_at_ms, updated_at_ms, last_accessed_ms, access_count, embedding_dim, embedding";

pub struct CandidateMemory {
    pub memory: StoredMemory,
    pub similarity: f32,
    pub keyword_score: f32,
    pub entity_score: f32,
    pub confidence_score: f32,
}

pub struct SqliteStore {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if path != Path::new(":memory:") {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }

        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                scope TEXT NOT NULL,
                content TEXT NOT NULL,
                summary TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                importance REAL NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                last_accessed_ms INTEGER,
                access_count INTEGER NOT NULL DEFAULT 0,
                embedding_dim INTEGER NOT NULL,
                embedding BLOB NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memories_scope_kind
                ON memories(scope, kind);

            CREATE INDEX IF NOT EXISTS idx_memories_created
                ON memories(created_at_ms DESC);

            CREATE TABLE IF NOT EXISTS memory_entities (
                memory_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                entity TEXT NOT NULL,
                entity_kind TEXT NOT NULL,
                weight REAL NOT NULL,
                PRIMARY KEY(memory_id, entity),
                FOREIGN KEY(memory_id) REFERENCES memories(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_memory_entities_scope_entity
                ON memory_entities(scope, entity);

            CREATE INDEX IF NOT EXISTS idx_memory_entities_entity
                ON memory_entities(entity);

            CREATE TABLE IF NOT EXISTS memory_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT,
                scope TEXT NOT NULL,
                event_type TEXT NOT NULL,
                body TEXT NOT NULL,
                data_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memory_events_scope_created
                ON memory_events(scope, created_at_ms DESC);

            CREATE TABLE IF NOT EXISTS memory_versions (
                id TEXT PRIMARY KEY,
                memory_id TEXT NOT NULL,
                action TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memory_versions_memory_created
                ON memory_versions(memory_id, created_at_ms DESC);

            CREATE TABLE IF NOT EXISTS memory_relations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_memory_id TEXT NOT NULL,
                target_memory_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                weight REAL NOT NULL,
                data_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memory_relations_source
                ON memory_relations(source_memory_id, relation);

            CREATE TABLE IF NOT EXISTS workspaces (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                category TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS policies (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                memory_type TEXT,
                mode TEXT NOT NULL,
                retain_days INTEGER,
                metadata_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_policies_scope
                ON policies(scope, memory_type);

            CREATE TABLE IF NOT EXISTS snapshots (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                scope TEXT NOT NULL,
                data_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshots_name_scope
                ON snapshots(name, scope);

            CREATE TABLE IF NOT EXISTS memory_conflicts (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                old_memory_id TEXT NOT NULL,
                new_memory_id TEXT NOT NULL,
                status TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memory_conflicts_scope
                ON memory_conflicts(scope, created_at_ms DESC);

            CREATE TABLE IF NOT EXISTS memory_inbox (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                content TEXT NOT NULL,
                reason TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memory_inbox_scope_status
                ON memory_inbox(scope, status, created_at_ms DESC);
            "#,
        )?;

        Ok(Self {
            path: path.to_path_buf(),
            conn: Mutex::new(conn),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn upsert(&self, memory: &StoredMemory, embedding: &[f32]) -> Result<()> {
        let conn = self.lock()?;
        let metadata_json =
            serde_json::to_string(&metadata_for_storage(&memory.metadata, &memory.attributes))?;
        let embedding_blob = serialize_f32_vec(embedding);

        conn.execute(
            r#"
            INSERT INTO memories (
                id, kind, scope, content, summary, metadata_json, importance,
                created_at_ms, updated_at_ms, last_accessed_ms, access_count,
                embedding_dim, embedding
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                scope = excluded.scope,
                content = excluded.content,
                summary = excluded.summary,
                metadata_json = excluded.metadata_json,
                importance = excluded.importance,
                updated_at_ms = excluded.updated_at_ms,
                last_accessed_ms = excluded.last_accessed_ms,
                access_count = excluded.access_count,
                embedding_dim = excluded.embedding_dim,
                embedding = excluded.embedding
            "#,
            params![
                &memory.id,
                memory.kind.as_str(),
                &memory.scope,
                &memory.content,
                &memory.summary,
                metadata_json,
                memory.importance,
                memory.created_at.timestamp_millis(),
                memory.updated_at.timestamp_millis(),
                memory
                    .last_accessed_at
                    .as_ref()
                    .map(|value| value.timestamp_millis()),
                memory.access_count as i64,
                embedding.len() as i64,
                embedding_blob,
            ],
        )?;

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<StoredMemory>> {
        let conn = self.lock()?;
        let sql = format!("SELECT {SELECT_FIELDS} FROM memories WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let memory = stmt
            .query_row(params![id], |row| row_to_memory(row).map_err(into_sql_err))
            .optional()?;
        Ok(memory)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.lock()?;
        let changed = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    pub fn list(
        &self,
        scope: Option<&str>,
        limit: usize,
        include_inactive: bool,
    ) -> Result<Vec<StoredMemory>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 10_000);
        let mut sql = format!("SELECT {SELECT_FIELDS} FROM memories");
        let mut values = Vec::new();

        if let Some(scope) = scope {
            sql.push_str(" WHERE scope = ?1");
            values.push(scope.to_string());
        }

        sql.push_str(&format!(" ORDER BY created_at_ms DESC LIMIT {limit}"));

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut memories = Vec::new();

        while let Some(row) = rows.next()? {
            let memory = row_to_memory(row)?;
            if include_inactive || memory.attributes.status.is_recallable() {
                memories.push(memory);
            }
        }

        Ok(memories)
    }

    pub fn all_memories(
        &self,
        scope: Option<&str>,
        include_inactive: bool,
    ) -> Result<Vec<StoredMemory>> {
        self.list(scope, 100_000, include_inactive)
    }

    pub fn index_entities(&self, memory: &StoredMemory, entities: &[Entity]) -> Result<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM memory_entities WHERE memory_id = ?1",
            params![&memory.id],
        )?;

        for entity in entities {
            tx.execute(
                r#"
                INSERT OR REPLACE INTO memory_entities
                    (memory_id, scope, entity, entity_kind, weight)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    &memory.id,
                    &memory.scope,
                    &entity.name,
                    entity.kind.as_str(),
                    memory.importance.max(0.1)
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn entity_links(
        &self,
        scope: Option<&str>,
        entity: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EntityLink>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 10_000);
        let mut sql = String::from(
            "SELECT e.entity, e.entity_kind, e.scope, e.memory_id, m.summary, e.weight \
             FROM memory_entities e \
             JOIN memories m ON m.id = e.memory_id",
        );
        let mut clauses = Vec::new();
        let mut values = Vec::new();

        if let Some(scope) = scope {
            clauses.push("e.scope = ?".to_string());
            values.push(scope.to_string());
        }

        if let Some(entity) = entity {
            clauses.push("LOWER(e.entity) = LOWER(?)".to_string());
            values.push(entity.to_string());
        }

        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }

        sql.push_str(&format!(
            " ORDER BY e.weight DESC, m.updated_at_ms DESC LIMIT {limit}"
        ));

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut links = Vec::new();

        while let Some(row) = rows.next()? {
            let kind_raw: String = row.get(1)?;
            links.push(EntityLink {
                entity: Entity {
                    name: row.get(0)?,
                    kind: parse_entity_kind(&kind_raw),
                },
                scope: row.get(2)?,
                memory_id: row.get(3)?,
                memory_summary: row.get(4)?,
                weight: row.get::<_, f32>(5)?,
            });
        }

        Ok(links)
    }

    pub fn search(
        &self,
        query_embedding: &[f32],
        query: &RecallQuery,
        pool_size: usize,
    ) -> Result<Vec<CandidateMemory>> {
        let conn = self.lock()?;
        let mut sql = format!("SELECT {SELECT_FIELDS} FROM memories");
        let mut clauses = Vec::new();
        let mut values = Vec::new();

        if let Some(scope) = &query.scope {
            let mut scopes = vec![scope.clone()];
            if query.include_global {
                scopes.extend(global_workspace_names(&conn)?);
                if !scopes.iter().any(|value| value == "global") {
                    scopes.push("global".to_string());
                }
            }
            scopes.sort();
            scopes.dedup();
            let placeholders = scopes.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            clauses.push(format!("scope IN ({placeholders})"));
            values.extend(scopes);
        }

        if !query.kinds.is_empty() {
            let placeholders = query
                .kinds
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            clauses.push(format!("kind IN ({placeholders})"));
            values.extend(query.kinds.iter().map(|kind| kind.as_str().to_string()));
        }

        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }

        sql.push_str(" ORDER BY updated_at_ms DESC");

        let query_tokens = tokenize(&query.text);
        let query_entities = extract_entities(&query.text)
            .into_iter()
            .map(|entity| entity.name.to_ascii_lowercase())
            .collect::<Vec<_>>();

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let pool_size = pool_size.max(query.limit).max(1);
        let mut candidates: Vec<CandidateMemory> = Vec::with_capacity(pool_size.min(256));

        while let Some(row) = rows.next()? {
            let memory = row_to_memory(row)?;

            if !query.include_inactive && !memory.attributes.status.is_recallable() {
                continue;
            }

            if let Some(expires_at) = memory.attributes.expires_at {
                if expires_at <= Utc::now() {
                    continue;
                }
            }

            if !query.tags.is_empty()
                && !query
                    .tags
                    .iter()
                    .all(|tag| memory.attributes.tags.iter().any(|value| value == tag))
            {
                continue;
            }

            let embedding_dim: i64 = row.get(11)?;
            if embedding_dim as usize != query_embedding.len() {
                continue;
            }

            let blob: Vec<u8> = row.get(12)?;
            let embedding = deserialize_f32_vec(&blob)?;
            let similarity = cosine_similarity(query_embedding, &embedding)?;
            let keyword_score = keyword_match_score(&query_tokens, &memory);
            let entity_score = entity_match_score(&query_entities, &memory);
            let confidence_score = memory.attributes.confidence.clamp(0.0, 1.0);

            let candidate = CandidateMemory {
                memory,
                similarity,
                keyword_score,
                entity_score,
                confidence_score,
            };

            if candidates.len() < pool_size {
                candidates.push(candidate);
                continue;
            }

            if let Some((min_index, min_candidate)) = candidates
                .iter()
                .enumerate()
                .min_by(|left, right| candidate_ordering(left.1, right.1))
            {
                if candidate_ordering(&candidate, min_candidate).is_gt() {
                    candidates[min_index] = candidate;
                }
            }
        }

        Ok(candidates)
    }

    pub fn mark_accessed(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let now = Utc::now().timestamp_millis();

        for id in ids {
            tx.execute(
                "UPDATE memories SET last_accessed_ms = ?1, access_count = access_count + 1 WHERE id = ?2",
                params![now, id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn record_event(
        &self,
        scope: &str,
        memory_id: Option<&str>,
        event_type: &str,
        body: &str,
        data: &Value,
    ) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO memory_events (memory_id, scope, event_type, body, data_json, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                memory_id,
                scope,
                event_type,
                body,
                serde_json::to_string(data)?,
                Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn timeline(
        &self,
        scope: Option<&str>,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TimelineEvent>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 10_000);
        let mut sql = String::from(
            "SELECT id, memory_id, scope, event_type, body, data_json, created_at_ms FROM memory_events",
        );
        let mut clauses = Vec::new();
        let mut values = Vec::new();

        if let Some(scope) = scope {
            clauses.push("scope = ?".to_string());
            values.push(scope.to_string());
        }

        if let Some(query) = query {
            clauses
                .push("(LOWER(body) LIKE LOWER(?) OR LOWER(data_json) LIKE LOWER(?))".to_string());
            let pattern = format!("%{query}%");
            values.push(pattern.clone());
            values.push(pattern);
        }

        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }

        sql.push_str(&format!(" ORDER BY created_at_ms DESC LIMIT {limit}"));
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut events = Vec::new();

        while let Some(row) = rows.next()? {
            events.push(TimelineEvent {
                id: row.get::<_, i64>(0)?.to_string(),
                memory_id: row.get(1)?,
                scope: row.get(2)?,
                event_type: row.get(3)?,
                body: row.get(4)?,
                data: serde_json::from_str::<Value>(&row.get::<_, String>(5)?)?,
                created_at: from_millis(row.get(6)?)
                    .ok_or_else(|| MemoryError::Storage("invalid event timestamp".to_string()))?,
            });
        }

        Ok(events)
    }

    pub fn latest_event(
        &self,
        event_type: &str,
        scope: Option<&str>,
    ) -> Result<Option<TimelineEvent>> {
        let conn = self.lock()?;
        let mut sql = String::from(
            "SELECT id, memory_id, scope, event_type, body, data_json, created_at_ms FROM memory_events WHERE event_type = ?1",
        );
        let mut values = vec![event_type.to_string()];
        if let Some(scope) = scope {
            sql.push_str(" AND scope = ?2");
            values.push(scope.to_string());
        }
        sql.push_str(" ORDER BY created_at_ms DESC LIMIT 1");

        let mut stmt = conn.prepare(&sql)?;
        let event = stmt
            .query_row(params_from_iter(values.iter()), |row| {
                let data_json: String = row.get(5)?;
                let data = serde_json::from_str::<Value>(&data_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                let created_at = from_millis(row.get(6)?).ok_or_else(|| {
                    into_sql_err(MemoryError::Storage("invalid event timestamp".to_string()))
                })?;
                Ok(TimelineEvent {
                    id: row.get::<_, i64>(0)?.to_string(),
                    memory_id: row.get(1)?,
                    scope: row.get(2)?,
                    event_type: row.get(3)?,
                    body: row.get(4)?,
                    data,
                    created_at,
                })
            })
            .optional()?;
        Ok(event)
    }

    pub fn record_version(&self, version: &MemoryVersion) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO memory_versions (id, memory_id, action, snapshot_json, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                &version.id,
                &version.memory_id,
                &version.action,
                serde_json::to_string(&version.snapshot)?,
                version.created_at.timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn list_versions(&self, memory_id: &str, limit: usize) -> Result<Vec<MemoryVersion>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 1_000);
        let mut stmt = conn.prepare(
            "SELECT id, memory_id, action, snapshot_json, created_at_ms
             FROM memory_versions
             WHERE memory_id = ?1
             ORDER BY created_at_ms DESC
             LIMIT ?2",
        )?;
        let mut rows = stmt.query(params![memory_id, limit as i64])?;
        let mut versions = Vec::new();
        while let Some(row) = rows.next()? {
            let snapshot_json: String = row.get(3)?;
            versions.push(MemoryVersion {
                id: row.get(0)?,
                memory_id: row.get(1)?,
                action: row.get(2)?,
                snapshot: serde_json::from_str(&snapshot_json)?,
                created_at: from_millis(row.get(4)?)
                    .ok_or_else(|| MemoryError::Storage("invalid version timestamp".to_string()))?,
            });
        }
        Ok(versions)
    }

    pub fn add_relation(
        &self,
        source_memory_id: &str,
        target_memory_id: &str,
        relation: &str,
        weight: f32,
        data: &Value,
    ) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO memory_relations
                (source_memory_id, target_memory_id, relation, weight, data_json, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                source_memory_id,
                target_memory_id,
                relation,
                weight,
                serde_json::to_string(data)?,
                Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn list_relations(&self, scope: Option<&str>, limit: usize) -> Result<Vec<MemoryRelation>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 10_000);
        let mut sql = String::from(
            "SELECT r.id, r.source_memory_id, r.target_memory_id, r.relation, r.weight, r.data_json, r.created_at_ms \
             FROM memory_relations r \
             JOIN memories s ON s.id = r.source_memory_id \
             JOIN memories t ON t.id = r.target_memory_id",
        );
        let mut values = Vec::new();
        if let Some(scope) = scope {
            sql.push_str(" WHERE s.scope = ?1 OR t.scope = ?1");
            values.push(scope.to_string());
        }
        sql.push_str(&format!(" ORDER BY r.created_at_ms DESC LIMIT {limit}"));
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut relations = Vec::new();
        while let Some(row) = rows.next()? {
            relations.push(MemoryRelation {
                id: row.get::<_, i64>(0)?.to_string(),
                source_memory_id: row.get(1)?,
                target_memory_id: row.get(2)?,
                relation: row.get(3)?,
                weight: row.get(4)?,
                data: serde_json::from_str(&row.get::<_, String>(5)?)?,
                created_at: from_millis(row.get(6)?).ok_or_else(|| {
                    MemoryError::Storage("invalid relation timestamp".to_string())
                })?,
            });
        }
        Ok(relations)
    }

    pub fn list_conflicts(&self, scope: Option<&str>, limit: usize) -> Result<Vec<ConflictRecord>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 1_000);
        let mut sql = String::from(
            "SELECT id, scope, old_memory_id, new_memory_id, status, reason, created_at_ms FROM memory_conflicts",
        );
        let mut values = Vec::new();
        if scope.is_some() {
            sql.push_str(" WHERE scope = ?1");
            values.push(scope.unwrap_or_default().to_string());
        }
        sql.push_str(&format!(" ORDER BY created_at_ms DESC LIMIT {limit}"));

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut conflicts = Vec::new();
        while let Some(row) = rows.next()? {
            conflicts.push(ConflictRecord {
                id: row.get(0)?,
                scope: row.get(1)?,
                old_memory_id: row.get(2)?,
                new_memory_id: row.get(3)?,
                status: row.get(4)?,
                reason: row.get(5)?,
                created_at: from_millis(row.get(6)?).ok_or_else(|| {
                    MemoryError::Storage("invalid conflict timestamp".to_string())
                })?,
            });
        }
        Ok(conflicts)
    }

    pub fn record_conflict(&self, conflict: &ConflictRecord) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO memory_conflicts
                (id, scope, old_memory_id, new_memory_id, status, reason, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                &conflict.id,
                &conflict.scope,
                &conflict.old_memory_id,
                &conflict.new_memory_id,
                &conflict.status,
                &conflict.reason,
                conflict.created_at.timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn save_snapshot(&self, snapshot: &SnapshotRecord) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO snapshots (id, name, scope, data_json, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                &snapshot.id,
                &snapshot.name,
                &snapshot.scope,
                serde_json::to_string(&snapshot.memories)?,
                snapshot.created_at.timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn snapshot_by_name(&self, scope: &str, name: &str) -> Result<Option<SnapshotRecord>> {
        let conn = self.lock()?;
        let row: Option<(String, String, String, String, i64)> = conn
            .query_row(
                "SELECT id, name, scope, data_json, created_at_ms FROM snapshots WHERE scope = ?1 AND name = ?2",
                params![scope, name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;

        row.map(|(id, name, scope, data_json, created_at_ms)| {
            Ok(SnapshotRecord {
                id,
                name,
                scope,
                memories: serde_json::from_str(&data_json)?,
                created_at: from_millis(created_at_ms).ok_or_else(|| {
                    MemoryError::Storage("invalid snapshot timestamp".to_string())
                })?,
            })
        })
        .transpose()
    }

    pub fn list_snapshots(&self, scope: Option<&str>, limit: usize) -> Result<Vec<SnapshotRecord>> {
        let conn = self.lock()?;
        let limit = limit.clamp(1, 1_000);
        let mut sql =
            String::from("SELECT id, name, scope, data_json, created_at_ms FROM snapshots");
        let mut values = Vec::new();
        if let Some(scope) = scope {
            sql.push_str(" WHERE scope = ?1");
            values.push(scope.to_string());
        }
        sql.push_str(&format!(" ORDER BY created_at_ms DESC LIMIT {limit}"));

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut snapshots = Vec::new();
        while let Some(row) = rows.next()? {
            snapshots.push(SnapshotRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                scope: row.get(2)?,
                memories: serde_json::from_str(&row.get::<_, String>(3)?)?,
                created_at: from_millis(row.get(4)?).ok_or_else(|| {
                    MemoryError::Storage("invalid snapshot timestamp".to_string())
                })?,
            });
        }
        Ok(snapshots)
    }

    pub fn upsert_workspace(&self, workspace: &WorkspaceInfo) -> Result<()> {
        let conn = self.lock()?;
        if workspace.active {
            conn.execute("UPDATE workspaces SET active = 0", [])?;
        }
        conn.execute(
            r#"
            INSERT OR REPLACE INTO workspaces
                (name, description, category, metadata_json, created_at_ms, updated_at_ms, active)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                &workspace.name,
                &workspace.description,
                &workspace.category,
                serde_json::to_string(&workspace.metadata)?,
                workspace.created_at.timestamp_millis(),
                workspace.updated_at.timestamp_millis(),
                if workspace.active { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT name, description, category, metadata_json, created_at_ms, updated_at_ms, active FROM workspaces ORDER BY active DESC, updated_at_ms DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut workspaces = Vec::new();
        while let Some(row) = rows.next()? {
            workspaces.push(WorkspaceInfo {
                name: row.get(0)?,
                description: row.get(1)?,
                category: row.get(2)?,
                metadata: serde_json::from_str(&row.get::<_, String>(3)?)?,
                created_at: from_millis(row.get(4)?).ok_or_else(|| {
                    MemoryError::Storage("invalid workspace created timestamp".to_string())
                })?,
                updated_at: from_millis(row.get(5)?).ok_or_else(|| {
                    MemoryError::Storage("invalid workspace updated timestamp".to_string())
                })?,
                active: row.get::<_, i64>(6)? == 1,
            });
        }
        Ok(workspaces)
    }

    pub fn current_workspace(&self) -> Result<Option<WorkspaceInfo>> {
        Ok(self
            .list_workspaces()?
            .into_iter()
            .find(|workspace| workspace.active))
    }

    pub fn upsert_policy(&self, policy: &RetentionPolicy) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO policies
                (id, scope, memory_type, mode, retain_days, metadata_json, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                &policy.id,
                &policy.scope,
                &policy.memory_type,
                format!("{:?}", policy.mode).to_ascii_lowercase(),
                policy.retain_days.map(i64::from),
                serde_json::to_string(&policy.metadata)?,
                policy.created_at.timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn list_policies(&self, scope: Option<&str>) -> Result<Vec<RetentionPolicy>> {
        let conn = self.lock()?;
        let mut sql =
            String::from("SELECT id, scope, memory_type, mode, retain_days, metadata_json, created_at_ms FROM policies");
        let mut values = Vec::new();
        if let Some(scope) = scope {
            sql.push_str(" WHERE scope = ?1");
            values.push(scope.to_string());
        }
        sql.push_str(" ORDER BY created_at_ms DESC");

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut policies = Vec::new();
        while let Some(row) = rows.next()? {
            policies.push(RetentionPolicy {
                id: row.get(0)?,
                scope: row.get(1)?,
                memory_type: row.get(2)?,
                mode: parse_policy_mode(&row.get::<_, String>(3)?),
                retain_days: row.get::<_, Option<i64>>(4)?.map(|value| value as u32),
                metadata: serde_json::from_str(&row.get::<_, String>(5)?)?,
                created_at: from_millis(row.get(6)?)
                    .ok_or_else(|| MemoryError::Storage("invalid policy timestamp".to_string()))?,
            });
        }
        Ok(policies)
    }

    pub fn queue_inbox(&self, entry: &InboxEntry) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO memory_inbox
                (id, scope, content, reason, metadata_json, status, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                &entry.id,
                &entry.scope,
                &entry.content,
                &entry.reason,
                serde_json::to_string(&entry.metadata)?,
                &entry.status,
                entry.created_at.timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn list_inbox(&self, scope: Option<&str>, status: Option<&str>) -> Result<Vec<InboxEntry>> {
        let conn = self.lock()?;
        let mut sql = String::from(
            "SELECT id, scope, content, reason, metadata_json, status, created_at_ms FROM memory_inbox",
        );
        let mut clauses = Vec::new();
        let mut values = Vec::new();

        if let Some(scope) = scope {
            clauses.push("scope = ?".to_string());
            values.push(scope.to_string());
        }
        if let Some(status) = status {
            clauses.push("status = ?".to_string());
            values.push(status.to_string());
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY created_at_ms DESC");

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(InboxEntry {
                id: row.get(0)?,
                scope: row.get(1)?,
                content: row.get(2)?,
                reason: row.get(3)?,
                metadata: serde_json::from_str(&row.get::<_, String>(4)?)?,
                status: row.get(5)?,
                created_at: from_millis(row.get(6)?)
                    .ok_or_else(|| MemoryError::Storage("invalid inbox timestamp".to_string()))?,
            });
        }
        Ok(entries)
    }

    pub fn resolve_inbox(&self, id: &str, status: &str) -> Result<bool> {
        let conn = self.lock()?;
        let changed = conn.execute(
            "UPDATE memory_inbox SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(changed > 0)
    }

    pub fn stats(&self, embedding_model: &str) -> Result<MemoryStats> {
        let stale_memories = self
            .all_memories(None, true)?
            .into_iter()
            .filter(is_stale)
            .count() as u64;

        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                COUNT(*),
                COUNT(DISTINCT scope),
                COALESCE(SUM(LENGTH(content) + LENGTH(summary) + LENGTH(metadata_json) + LENGTH(embedding)), 0),
                MIN(created_at_ms),
                MAX(created_at_ms)
            FROM memories
            "#,
        )?;

        let (memories, workspaces, bytes, oldest, newest): (
            i64,
            i64,
            i64,
            Option<i64>,
            Option<i64>,
        ) = stmt.query_row([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;

        let avg_latency: f32 = conn
            .query_row(
                r#"
                SELECT COALESCE(AVG(CAST(json_extract(data_json, '$.latency_ms') AS REAL)), 0.0)
                FROM memory_events
                WHERE event_type = 'recall'
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let conflicts: i64 =
            conn.query_row("SELECT COUNT(*) FROM memory_conflicts", [], |row| {
                row.get(0)
            })?;

        let mut entity_stmt = conn.prepare(
            r#"
            SELECT entity, entity_kind, COUNT(*)
            FROM memory_entities
            GROUP BY entity, entity_kind
            ORDER BY COUNT(*) DESC, entity ASC
            LIMIT 8
            "#,
        )?;
        let mut entity_rows = entity_stmt.query([])?;
        let mut top_entities = Vec::new();
        while let Some(row) = entity_rows.next()? {
            top_entities.push(EntityStat {
                name: row.get(0)?,
                kind: row.get(1)?,
                count: row.get::<_, i64>(2)? as u64,
            });
        }

        Ok(MemoryStats {
            memories: memories as u64,
            workspaces: workspaces as u64,
            bytes: bytes as u64,
            oldest_memory_at: oldest.and_then(from_millis),
            newest_memory_at: newest.and_then(from_millis),
            embedding_model: embedding_model.to_string(),
            average_recall_latency_ms: avg_latency,
            stale_memories,
            conflicts: conflicts as u64,
            top_entities,
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| MemoryError::Storage("sqlite connection lock was poisoned".to_string()))
    }
}

fn row_to_memory(row: &Row<'_>) -> Result<StoredMemory> {
    let kind_raw: String = row.get(1)?;
    let metadata_json: String = row.get(5)?;
    let created_ms: i64 = row.get(7)?;
    let updated_ms: i64 = row.get(8)?;
    let last_accessed_ms: Option<i64> = row.get(9)?;
    let access_count: i64 = row.get(10)?;
    let metadata = serde_json::from_str::<Value>(&metadata_json)?;

    let kind = MemoryKind::from_str(&kind_raw)?;
    let content: String = row.get(3)?;
    let importance = row.get::<_, f32>(6)?.clamp(0.0, 1.0);
    let created_at = from_millis(created_ms)
        .ok_or_else(|| MemoryError::Storage("invalid created_at timestamp".to_string()))?;
    let updated_at = from_millis(updated_ms)
        .ok_or_else(|| MemoryError::Storage("invalid updated_at timestamp".to_string()))?;
    let attributes = attributes_from_metadata(&metadata);

    Ok(StoredMemory {
        id: row.get(0)?,
        kind,
        scope: row.get(2)?,
        content: content.clone(),
        summary: row.get(4)?,
        metadata: metadata.clone(),
        importance,
        created_at,
        updated_at,
        last_accessed_at: last_accessed_ms.and_then(from_millis),
        access_count: access_count.max(0) as u64,
        derived: derive_memory_scores(
            kind,
            &content,
            importance,
            created_at,
            updated_at,
            access_count.max(0) as u64,
            &attributes,
        ),
        attributes,
    })
}

fn from_millis(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(value).single()
}

fn parse_entity_kind(value: &str) -> EntityKind {
    match value {
        "person" => EntityKind::Person,
        "project" => EntityKind::Project,
        "file" => EntityKind::File,
        "tag" => EntityKind::Tag,
        "url" => EntityKind::Url,
        "code" => EntityKind::Code,
        _ => EntityKind::Concept,
    }
}

fn global_workspace_names(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM workspaces WHERE LOWER(category) = 'global' ORDER BY name")?;
    let mut rows = stmt.query([])?;
    let mut names = Vec::new();
    while let Some(row) = rows.next()? {
        names.push(row.get(0)?);
    }
    Ok(names)
}

fn parse_policy_mode(value: &str) -> crate::PolicyMode {
    match value {
        "decay" => crate::PolicyMode::Decay,
        "ephemeral" => crate::PolicyMode::Ephemeral,
        "session_only" => crate::PolicyMode::SessionOnly,
        "manual_review" => crate::PolicyMode::ManualReview,
        "never_store" => crate::PolicyMode::NeverStore,
        "reject" => crate::PolicyMode::Reject,
        _ => crate::PolicyMode::Forever,
    }
}

fn candidate_ordering(left: &CandidateMemory, right: &CandidateMemory) -> std::cmp::Ordering {
    left.similarity
        .partial_cmp(&right.similarity)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            left.keyword_score
                .partial_cmp(&right.keyword_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            left.entity_score
                .partial_cmp(&right.entity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 2)
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn keyword_match_score(query_tokens: &[String], memory: &StoredMemory) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let haystack = format!("{} {}", memory.summary, memory.content).to_ascii_lowercase();
    let matches = query_tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count() as f32;
    (matches / query_tokens.len() as f32).clamp(0.0, 1.0)
}

fn entity_match_score(query_entities: &[String], memory: &StoredMemory) -> f32 {
    if query_entities.is_empty() {
        return 0.0;
    }

    let memory_entities = extract_entities(&format!("{} {}", memory.summary, memory.content))
        .into_iter()
        .map(|entity| entity.name.to_ascii_lowercase())
        .collect::<Vec<_>>();

    if memory_entities.is_empty() {
        return 0.0;
    }

    let overlap = query_entities
        .iter()
        .filter(|entity| memory_entities.iter().any(|value| value == *entity))
        .count() as f32;

    (overlap / query_entities.len() as f32).clamp(0.0, 1.0)
}

fn is_stale(memory: &StoredMemory) -> bool {
    let age_days = (Utc::now() - memory.updated_at).num_days();
    age_days > 30 && memory.access_count == 0
}

fn into_sql_err(error: MemoryError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
