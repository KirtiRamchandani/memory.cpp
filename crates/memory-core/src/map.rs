use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    extract_entities, MemoryEngine, MemoryKind, MemoryRelation, Result, SnapshotRecord,
    StoredMemory, TimelineEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MapType {
    #[default]
    Evolution,
    Timeline,
    Decisions,
    Architecture,
    Bugs,
    Dependencies,
}

impl MapType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Evolution => "evolution",
            Self::Timeline => "timeline",
            Self::Decisions => "decisions",
            Self::Architecture => "architecture",
            Self::Bugs => "bugs",
            Self::Dependencies => "dependencies",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MapOutputFormat {
    Json,
    #[default]
    Markdown,
    Mermaid,
    Html,
}

impl MapOutputFormat {
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Mermaid => "text/plain; charset=utf-8",
            Self::Html => "text/html; charset=utf-8",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapNodeClass {
    Milestone,
    Decision,
    Bug,
    Fix,
    File,
    Component,
    Workspace,
    Entity,
    Source,
    Release,
}

impl MapNodeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Milestone => "milestone",
            Self::Decision => "decision",
            Self::Bug => "bug",
            Self::Fix => "fix",
            Self::File => "file",
            Self::Component => "component",
            Self::Workspace => "workspace",
            Self::Entity => "entity",
            Self::Source => "source",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapEdgeKind {
    DependsOn,
    IntroducedBy,
    FixedBy,
    Supersedes,
    Contradicts,
    Mentions,
    BelongsTo,
    CausedBy,
}

impl MapEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DependsOn => "depends_on",
            Self::IntroducedBy => "introduced_by",
            Self::FixedBy => "fixed_by",
            Self::Supersedes => "supersedes",
            Self::Contradicts => "contradicts",
            Self::Mentions => "mentions",
            Self::BelongsTo => "belongs_to",
            Self::CausedBy => "caused_by",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapRequest {
    pub path: Option<PathBuf>,
    pub project: Option<String>,
    pub workspace: Option<String>,
    pub map_type: MapType,
    pub output: MapOutputFormat,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub chronological: bool,
    pub why: bool,
    pub impact: Option<String>,
    pub limit: usize,
}

impl Default for MapRequest {
    fn default() -> Self {
        Self {
            path: None,
            project: None,
            workspace: None,
            map_type: MapType::Evolution,
            output: MapOutputFormat::Markdown,
            from: None,
            to: None,
            chronological: false,
            why: false,
            impact: None,
            limit: 48,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapCitation {
    pub id: String,
    pub label: String,
    pub memory_id: Option<String>,
    pub source_path: Option<String>,
    pub source_line: Option<u64>,
    pub source_commit: Option<String>,
    pub source_conversation_id: Option<String>,
    pub source_app: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapNode {
    pub id: String,
    pub class: MapNodeClass,
    pub label: String,
    pub detail: Option<String>,
    pub scope: Option<String>,
    pub memory_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub importance: Option<f32>,
    pub confidence: Option<f32>,
    pub badges: Vec<String>,
    pub citation_ids: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: MapEdgeKind,
    pub label: Option<String>,
    pub weight: f32,
    pub citation_ids: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMap {
    pub title: String,
    pub map_type: MapType,
    pub generated_at: DateTime<Utc>,
    pub workspace: Option<String>,
    pub project: Option<String>,
    pub source_path: Option<String>,
    pub notes: Vec<String>,
    pub summary: Vec<String>,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
    pub citations: Vec<MapCitation>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapNodeChange {
    pub node_id: String,
    pub before: MapNode,
    pub after: MapNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapDiff {
    pub left: String,
    pub right: String,
    pub added_nodes: Vec<MapNode>,
    pub removed_nodes: Vec<MapNode>,
    pub changed_nodes: Vec<MapNodeChange>,
    pub added_edges: Vec<MapEdge>,
    pub removed_edges: Vec<MapEdge>,
    pub notes: Vec<String>,
}

impl MemoryMap {
    pub fn render(&self, format: MapOutputFormat) -> Result<String> {
        let body = match format {
            MapOutputFormat::Json => serde_json::to_string_pretty(self)?,
            MapOutputFormat::Markdown => render_markdown(self),
            MapOutputFormat::Mermaid => render_mermaid(self),
            MapOutputFormat::Html => render_html(self),
        };
        Ok(body)
    }
}

impl MapDiff {
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Map diff: {} -> {}\n\n", self.left, self.right));

        if !self.notes.is_empty() {
            out.push_str("## Notes\n");
            for note in &self.notes {
                out.push_str(&format!("- {}\n", note));
            }
            out.push('\n');
        }

        out.push_str("## Added nodes\n");
        if self.added_nodes.is_empty() {
            out.push_str("- none\n");
        } else {
            for node in &self.added_nodes {
                out.push_str(&format!("- {} [{}]\n", node.label, node.class.as_str()));
            }
        }
        out.push('\n');

        out.push_str("## Removed nodes\n");
        if self.removed_nodes.is_empty() {
            out.push_str("- none\n");
        } else {
            for node in &self.removed_nodes {
                out.push_str(&format!("- {} [{}]\n", node.label, node.class.as_str()));
            }
        }
        out.push('\n');

        out.push_str("## Changed nodes\n");
        if self.changed_nodes.is_empty() {
            out.push_str("- none\n");
        } else {
            for change in &self.changed_nodes {
                out.push_str(&format!(
                    "- {} [{}]\n",
                    change.after.label,
                    change.after.class.as_str()
                ));
            }
        }
        out.push('\n');

        out.push_str("## Added edges\n");
        if self.added_edges.is_empty() {
            out.push_str("- none\n");
        } else {
            for edge in &self.added_edges {
                out.push_str(&format!(
                    "- {} -> {} ({})\n",
                    edge.source,
                    edge.target,
                    edge.kind.as_str()
                ));
            }
        }
        out.push('\n');

        out.push_str("## Removed edges\n");
        if self.removed_edges.is_empty() {
            out.push_str("- none\n");
        } else {
            for edge in &self.removed_edges {
                out.push_str(&format!(
                    "- {} -> {} ({})\n",
                    edge.source,
                    edge.target,
                    edge.kind.as_str()
                ));
            }
        }

        out
    }
}

#[derive(Default)]
struct MapAssembler {
    nodes: BTreeMap<String, MapNode>,
    edges: BTreeMap<String, MapEdge>,
    citations: BTreeMap<String, MapCitation>,
    notes: Vec<String>,
    summary: Vec<String>,
}

impl MapAssembler {
    fn add_node(&mut self, node: MapNode) {
        self.nodes.entry(node.id.clone()).or_insert(node);
    }

    fn add_edge(&mut self, edge: MapEdge) {
        self.edges.entry(edge.id.clone()).or_insert(edge);
    }

    fn add_citation(&mut self, citation: MapCitation) {
        self.citations
            .entry(citation.id.clone())
            .or_insert(citation);
    }

    fn note(&mut self, note: impl Into<String>) {
        let note = note.into();
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }

    fn summary_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        if !self.summary.contains(&line) {
            self.summary.push(line);
        }
    }
}

#[derive(Default)]
struct DocInfo {
    path: String,
    title: String,
    summary: String,
}

#[derive(Default)]
struct GitInfo {
    notes: Vec<String>,
    commits: Vec<GitCommit>,
    releases: Vec<GitRelease>,
}

#[derive(Clone)]
struct GitCommit {
    sha: String,
    title: String,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
struct GitRelease {
    name: String,
    commit: Option<String>,
}

#[derive(Clone)]
struct DependencyInfo {
    ecosystem: String,
    name: String,
}

impl MemoryEngine {
    pub fn build_map(&self, request: &MapRequest) -> Result<MemoryMap> {
        let workspace = request.workspace.clone().or_else(|| {
            self.current_workspace()
                .ok()
                .flatten()
                .map(|workspace| workspace.name)
        });
        let memories = self.filtered_memories(workspace.as_deref(), request)?;
        let events = self.filtered_events(workspace.as_deref(), request)?;
        let relations = self.filtered_relations(workspace.as_deref(), request, &memories)?;
        self.assemble_map(request, workspace, memories, events, relations, None)
    }

    pub fn compare_maps(&self, request: &MapRequest, left: &str, right: &str) -> Result<MapDiff> {
        let left_map = self.load_map_reference(request, left)?;
        let right_map = self.load_map_reference(request, right)?;

        let left_nodes = left_map
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node))
            .collect::<HashMap<_, _>>();
        let right_nodes = right_map
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node))
            .collect::<HashMap<_, _>>();
        let left_edges = left_map
            .edges
            .iter()
            .map(|edge| (edge.id.clone(), edge))
            .collect::<HashMap<_, _>>();
        let right_edges = right_map
            .edges
            .iter()
            .map(|edge| (edge.id.clone(), edge))
            .collect::<HashMap<_, _>>();

        let mut added_nodes = Vec::new();
        let mut removed_nodes = Vec::new();
        let mut changed_nodes = Vec::new();
        let mut added_edges = Vec::new();
        let mut removed_edges = Vec::new();

        for node in &right_map.nodes {
            match left_nodes.get(&node.id) {
                Some(old) if nodes_equivalent(old, node) => {}
                Some(old) => changed_nodes.push(MapNodeChange {
                    node_id: node.id.clone(),
                    before: (*old).clone(),
                    after: node.clone(),
                }),
                None => added_nodes.push(node.clone()),
            }
        }

        for node in &left_map.nodes {
            if !right_nodes.contains_key(&node.id) {
                removed_nodes.push(node.clone());
            }
        }

        for edge in &right_map.edges {
            if !left_edges.contains_key(&edge.id) {
                added_edges.push(edge.clone());
            }
        }

        for edge in &left_map.edges {
            if !right_edges.contains_key(&edge.id) {
                removed_edges.push(edge.clone());
            }
        }

        let mut notes = Vec::new();
        notes.push(format!(
            "compared {} nodes/{} edges against {} nodes/{} edges",
            left_map.nodes.len(),
            left_map.edges.len(),
            right_map.nodes.len(),
            right_map.edges.len()
        ));
        if request.path.is_none() {
            notes.push(
                "map diff used workspace memory and optional snapshots; docs/git enrichment was pathless"
                    .to_string(),
            );
        }

        Ok(MapDiff {
            left: left.to_string(),
            right: right.to_string(),
            added_nodes,
            removed_nodes,
            changed_nodes,
            added_edges,
            removed_edges,
            notes,
        })
    }

    fn load_map_reference(&self, request: &MapRequest, name: &str) -> Result<MemoryMap> {
        let candidate_path = PathBuf::from(name);
        if candidate_path.exists() {
            let value = fs::read_to_string(&candidate_path)?;
            return Ok(serde_json::from_str(&value)?);
        }

        if name.eq_ignore_ascii_case("now") {
            return self.build_map(request);
        }

        let workspace = request
            .workspace
            .clone()
            .or_else(|| {
                self.current_workspace()
                    .ok()
                    .flatten()
                    .map(|workspace| workspace.name)
            })
            .unwrap_or_else(|| "default".to_string());
        let snapshot = self
            .snapshot_named(&workspace, name)?
            .ok_or_else(|| crate::MemoryError::NotFound(format!("snapshot '{name}'")))?;
        self.build_map_from_snapshot(request, snapshot)
    }

    fn build_map_from_snapshot(
        &self,
        request: &MapRequest,
        snapshot: SnapshotRecord,
    ) -> Result<MemoryMap> {
        let memories = snapshot
            .memories
            .into_iter()
            .filter(|memory| memory_in_range(memory, request.from.as_ref(), request.to.as_ref()))
            .collect::<Vec<_>>();
        let ids = memories
            .iter()
            .map(|memory| memory.id.clone())
            .collect::<HashSet<_>>();
        let events = self
            .timeline(Some(&snapshot.scope), None, request.limit.max(24) * 6)?
            .into_iter()
            .filter(|event| {
                event_in_range(event, request.from.as_ref(), request.to.as_ref())
                    && event
                        .memory_id
                        .as_ref()
                        .map(|memory_id| ids.contains(memory_id))
                        .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        let relations = self
            .relations(Some(&snapshot.scope), request.limit.max(24) * 8)?
            .into_iter()
            .filter(|relation| {
                ids.contains(&relation.source_memory_id) || ids.contains(&relation.target_memory_id)
            })
            .collect::<Vec<_>>();
        self.assemble_map(
            request,
            Some(snapshot.scope.clone()),
            memories,
            events,
            relations,
            Some(format!("snapshot {}", snapshot.name)),
        )
    }

    fn filtered_memories(
        &self,
        workspace: Option<&str>,
        request: &MapRequest,
    ) -> Result<Vec<StoredMemory>> {
        Ok(self
            .all_memories(workspace, true)?
            .into_iter()
            .filter(|memory| memory_in_range(memory, request.from.as_ref(), request.to.as_ref()))
            .collect())
    }

    fn filtered_events(
        &self,
        workspace: Option<&str>,
        request: &MapRequest,
    ) -> Result<Vec<TimelineEvent>> {
        Ok(self
            .timeline(workspace, None, request.limit.max(24) * 6)?
            .into_iter()
            .filter(|event| event_in_range(event, request.from.as_ref(), request.to.as_ref()))
            .collect())
    }

    fn filtered_relations(
        &self,
        workspace: Option<&str>,
        request: &MapRequest,
        memories: &[StoredMemory],
    ) -> Result<Vec<MemoryRelation>> {
        let ids = memories
            .iter()
            .map(|memory| memory.id.as_str())
            .collect::<HashSet<_>>();
        Ok(self
            .relations(workspace, request.limit.max(24) * 8)?
            .into_iter()
            .filter(|relation| {
                ids.contains(relation.source_memory_id.as_str())
                    || ids.contains(relation.target_memory_id.as_str())
            })
            .collect())
    }

    fn assemble_map(
        &self,
        request: &MapRequest,
        workspace: Option<String>,
        memories: Vec<StoredMemory>,
        events: Vec<TimelineEvent>,
        relations: Vec<MemoryRelation>,
        snapshot_note: Option<String>,
    ) -> Result<MemoryMap> {
        let project = request
            .project
            .clone()
            .or_else(|| infer_project_name(request.path.as_deref()));
        let mut assembler = MapAssembler::default();

        if let Some(note) = snapshot_note {
            assembler.note(format!("built from {}", note));
        }

        let docs = request
            .path
            .as_deref()
            .map(|path| scan_docs(path, request.limit.max(8)))
            .transpose()?
            .unwrap_or_default();
        let git = request
            .path
            .as_deref()
            .map(|path| scan_git(path, request.limit.max(16)))
            .transpose()?
            .unwrap_or_else(|| GitInfo {
                notes: vec![
                    "git enrichment unavailable because no source path was supplied".into(),
                ],
                ..Default::default()
            });
        let dependencies = request
            .path
            .as_deref()
            .map(scan_dependencies)
            .transpose()?
            .unwrap_or_default();

        for note in &git.notes {
            assembler.note(note.clone());
        }

        let project_node_id = project
            .as_ref()
            .map(|name| format!("project:{}", sanitize_id(name)))
            .unwrap_or_else(|| "project:memory-map".to_string());
        assembler.add_node(MapNode {
            id: project_node_id.clone(),
            class: MapNodeClass::Component,
            label: project
                .clone()
                .unwrap_or_else(|| "project evolution".to_string()),
            detail: request
                .path
                .as_ref()
                .map(|path| format!("derived from {}", path.display())),
            scope: workspace.clone(),
            memory_id: None,
            created_at: None,
            updated_at: None,
            importance: Some(1.0),
            confidence: Some(1.0),
            badges: vec!["project".to_string()],
            citation_ids: Vec::new(),
            metadata: json!({ "kind": "project_root" }),
        });

        if let Some(scope) = workspace.clone() {
            let workspace_node_id = format!("workspace:{}", sanitize_id(&scope));
            assembler.add_node(MapNode {
                id: workspace_node_id.clone(),
                class: MapNodeClass::Workspace,
                label: scope.clone(),
                detail: Some("workspace memory scope".to_string()),
                scope: Some(scope.clone()),
                memory_id: None,
                created_at: None,
                updated_at: None,
                importance: Some(0.95),
                confidence: Some(1.0),
                badges: vec!["scope".to_string()],
                citation_ids: Vec::new(),
                metadata: json!({ "workspace": scope }),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", workspace_node_id, project_node_id),
                source: workspace_node_id,
                target: project_node_id.clone(),
                kind: MapEdgeKind::BelongsTo,
                label: Some("belongs to project".to_string()),
                weight: 1.0,
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
        }

        self.add_doc_nodes(&mut assembler, &project_node_id, &docs);
        self.add_dependency_nodes(&mut assembler, &project_node_id, &dependencies);

        match request.map_type {
            MapType::Evolution | MapType::Timeline => {
                self.add_timeline_nodes(
                    &mut assembler,
                    request,
                    &project_node_id,
                    &memories,
                    &events,
                    &git,
                );
            }
            MapType::Decisions => {
                self.add_decision_nodes(
                    &mut assembler,
                    request,
                    &project_node_id,
                    &memories,
                    &relations,
                );
            }
            MapType::Architecture => {
                self.add_architecture_nodes(
                    &mut assembler,
                    request,
                    &project_node_id,
                    &memories,
                    &relations,
                    &dependencies,
                );
            }
            MapType::Bugs => {
                self.add_bug_nodes(
                    &mut assembler,
                    request,
                    &project_node_id,
                    &memories,
                    &events,
                    &relations,
                );
            }
            MapType::Dependencies => {
                self.add_dependency_map_nodes(
                    &mut assembler,
                    request,
                    &project_node_id,
                    &memories,
                    &dependencies,
                );
            }
        }

        if request.chronological
            || matches!(request.map_type, MapType::Evolution | MapType::Timeline)
        {
            add_chronological_edges(&mut assembler);
        }

        if let Some(target) = request.impact.as_deref() {
            filter_to_impact(&mut assembler, target);
            assembler.note(format!(
                "filtered map to impact neighborhood for '{target}'"
            ));
        }

        let mut map = MemoryMap {
            title: format!(
                "{} {} map",
                project.clone().unwrap_or_else(|| "memory.cpp".to_string()),
                request.map_type.as_str()
            ),
            map_type: request.map_type,
            generated_at: Utc::now(),
            workspace,
            project,
            source_path: request.path.as_ref().map(|path| path.display().to_string()),
            notes: assembler.notes,
            summary: assembler.summary,
            nodes: assembler.nodes.into_values().collect(),
            edges: assembler.edges.into_values().collect(),
            citations: assembler.citations.into_values().collect(),
            metadata: json!({
                "output": request.output,
                "why": request.why,
                "chronological": request.chronological,
                "limit": request.limit,
            }),
        };

        map.nodes.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.class.as_str().cmp(right.class.as_str()))
                .then_with(|| left.label.cmp(&right.label))
        });
        map.edges.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
        });
        map.citations.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.id.cmp(&right.id))
        });

        if map.summary.is_empty() {
            map.summary.push(format!(
                "{} nodes, {} edges, {} citations",
                map.nodes.len(),
                map.edges.len(),
                map.citations.len()
            ));
        }

        Ok(map)
    }

    fn add_doc_nodes(&self, assembler: &mut MapAssembler, project_node_id: &str, docs: &[DocInfo]) {
        for doc in docs.iter().take(12) {
            let citation_id = format!("citation:{}", sanitize_id(&doc.path));
            assembler.add_citation(MapCitation {
                id: citation_id.clone(),
                label: doc.title.clone(),
                memory_id: None,
                source_path: Some(doc.path.clone()),
                source_line: None,
                source_commit: None,
                source_conversation_id: None,
                source_app: Some("docs".to_string()),
                snippet: Some(doc.summary.clone()),
            });
            let node_id = format!("source:{}", sanitize_id(&doc.path));
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class: MapNodeClass::Source,
                label: doc.title.clone(),
                detail: Some(doc.summary.clone()),
                scope: None,
                memory_id: None,
                created_at: None,
                updated_at: None,
                importance: Some(0.7),
                confidence: Some(0.8),
                badges: vec!["doc".to_string()],
                citation_ids: vec![citation_id.clone()],
                metadata: json!({ "path": doc.path }),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", node_id, project_node_id),
                source: node_id,
                target: project_node_id.to_string(),
                kind: MapEdgeKind::BelongsTo,
                label: Some("documents project".to_string()),
                weight: 0.7,
                citation_ids: vec![citation_id],
                metadata: json!({}),
            });
        }
    }

    fn add_dependency_nodes(
        &self,
        assembler: &mut MapAssembler,
        project_node_id: &str,
        dependencies: &[DependencyInfo],
    ) {
        for dependency in dependencies.iter().take(32) {
            let node_id = format!(
                "dep:{}:{}",
                sanitize_id(&dependency.ecosystem),
                sanitize_id(&dependency.name)
            );
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class: MapNodeClass::Component,
                label: dependency.name.clone(),
                detail: Some(format!("{} dependency", dependency.ecosystem)),
                scope: None,
                memory_id: None,
                created_at: None,
                updated_at: None,
                importance: Some(0.6),
                confidence: Some(0.8),
                badges: vec![dependency.ecosystem.clone(), "dependency".to_string()],
                citation_ids: Vec::new(),
                metadata: json!({ "ecosystem": dependency.ecosystem }),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, node_id),
                source: project_node_id.to_string(),
                target: node_id,
                kind: MapEdgeKind::DependsOn,
                label: Some("depends on".to_string()),
                weight: 0.85,
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
        }
    }

    fn add_timeline_nodes(
        &self,
        assembler: &mut MapAssembler,
        request: &MapRequest,
        project_node_id: &str,
        memories: &[StoredMemory],
        events: &[TimelineEvent],
        git: &GitInfo,
    ) {
        for event in events.iter().take(request.limit.max(12) * 2) {
            let node_id = format!("event:{}", event.id);
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class: MapNodeClass::Milestone,
                label: event.body.clone(),
                detail: Some(format!("{} event", event.event_type)),
                scope: Some(event.scope.clone()),
                memory_id: event.memory_id.clone(),
                created_at: Some(event.created_at),
                updated_at: Some(event.created_at),
                importance: Some(0.65),
                confidence: Some(0.8),
                badges: vec![event.event_type.clone()],
                citation_ids: Vec::new(),
                metadata: event.data.clone(),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, node_id),
                source: project_node_id.to_string(),
                target: node_id,
                kind: MapEdgeKind::IntroducedBy,
                label: Some("timeline event".to_string()),
                weight: 0.65,
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
        }

        for memory in memories
            .iter()
            .filter(|memory| include_memory_in_map(request.map_type, memory))
            .take(request.limit.max(8))
        {
            add_memory_node(assembler, memory, request.why);
            let citation_ids = citations_for_memory(assembler, memory);
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, memory.id),
                source: project_node_id.to_string(),
                target: memory.id.clone(),
                kind: MapEdgeKind::IntroducedBy,
                label: Some("important memory".to_string()),
                weight: memory.importance,
                citation_ids,
                metadata: json!({}),
            });
        }

        for commit in git.commits.iter().take(16) {
            let node_id = format!("commit:{}", &commit.sha[..commit.sha.len().min(12)]);
            let citation_id = format!("citation:commit:{}", sanitize_id(&commit.sha));
            assembler.add_citation(MapCitation {
                id: citation_id.clone(),
                label: commit.title.clone(),
                memory_id: None,
                source_path: None,
                source_line: None,
                source_commit: Some(commit.sha.clone()),
                source_conversation_id: None,
                source_app: Some("git".to_string()),
                snippet: Some(commit.title.clone()),
            });
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class: MapNodeClass::Release,
                label: commit.title.clone(),
                detail: Some(format!(
                    "git commit {}",
                    &commit.sha[..commit.sha.len().min(7)]
                )),
                scope: None,
                memory_id: None,
                created_at: commit.created_at,
                updated_at: commit.created_at,
                importance: Some(0.55),
                confidence: Some(0.78),
                badges: vec!["git".to_string()],
                citation_ids: vec![citation_id.clone()],
                metadata: json!({ "sha": commit.sha }),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, node_id),
                source: project_node_id.to_string(),
                target: node_id,
                kind: MapEdgeKind::IntroducedBy,
                label: Some("git history".to_string()),
                weight: 0.55,
                citation_ids: vec![citation_id],
                metadata: json!({}),
            });
        }

        for release in git.releases.iter().take(8) {
            let node_id = format!("release:{}", sanitize_id(&release.name));
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class: MapNodeClass::Release,
                label: release.name.clone(),
                detail: Some("git tag / release marker".to_string()),
                scope: None,
                memory_id: None,
                created_at: None,
                updated_at: None,
                importance: Some(0.7),
                confidence: Some(0.82),
                badges: vec!["release".to_string()],
                citation_ids: Vec::new(),
                metadata: json!({ "commit": release.commit }),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, node_id),
                source: project_node_id.to_string(),
                target: node_id,
                kind: MapEdgeKind::IntroducedBy,
                label: Some("release".to_string()),
                weight: 0.72,
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
        }

        if request.why {
            assembler.summary_line(
                "why mode surfaces the reasons behind milestones using decision memory and source citations",
            );
        }
    }

    fn add_decision_nodes(
        &self,
        assembler: &mut MapAssembler,
        request: &MapRequest,
        project_node_id: &str,
        memories: &[StoredMemory],
        relations: &[MemoryRelation],
    ) {
        let decisions = memories
            .iter()
            .filter(|memory| is_decision_memory(memory))
            .take(request.limit.max(8))
            .collect::<Vec<_>>();
        for memory in decisions {
            add_memory_node(assembler, memory, request.why);
            let citation_ids = citations_for_memory(assembler, memory);
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, memory.id),
                source: project_node_id.to_string(),
                target: memory.id.clone(),
                kind: MapEdgeKind::IntroducedBy,
                label: Some("decision".to_string()),
                weight: memory.importance.max(0.5),
                citation_ids: citation_ids.clone(),
                metadata: json!({}),
            });
            for entity in related_file_entities(memory).into_iter().take(6) {
                let entity_node = format!("file:{}", sanitize_id(&entity));
                assembler.add_node(MapNode {
                    id: entity_node.clone(),
                    class: MapNodeClass::File,
                    label: entity.clone(),
                    detail: Some("related file or code surface".to_string()),
                    scope: Some(memory.scope.clone()),
                    memory_id: None,
                    created_at: None,
                    updated_at: None,
                    importance: Some(0.55),
                    confidence: Some(0.7),
                    badges: vec!["related".to_string()],
                    citation_ids: Vec::new(),
                    metadata: json!({}),
                });
                assembler.add_edge(MapEdge {
                    id: format!("edge:{}:{}", memory.id, entity_node),
                    source: memory.id.clone(),
                    target: entity_node,
                    kind: MapEdgeKind::Mentions,
                    label: Some("touches file".to_string()),
                    weight: 0.65,
                    citation_ids: citation_ids.clone(),
                    metadata: json!({}),
                });
            }
        }
        add_relation_edges(assembler, relations, memories);
    }

    fn add_architecture_nodes(
        &self,
        assembler: &mut MapAssembler,
        request: &MapRequest,
        project_node_id: &str,
        memories: &[StoredMemory],
        relations: &[MemoryRelation],
        dependencies: &[DependencyInfo],
    ) {
        let mut components = BTreeSet::new();
        for memory in memories.iter().take(request.limit.max(12) * 2) {
            if !include_memory_in_map(MapType::Architecture, memory) {
                continue;
            }
            add_memory_node(assembler, memory, request.why);
            let citation_ids = citations_for_memory(assembler, memory);
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, memory.id),
                source: project_node_id.to_string(),
                target: memory.id.clone(),
                kind: MapEdgeKind::BelongsTo,
                label: Some("architecture memory".to_string()),
                weight: memory.importance,
                citation_ids,
                metadata: json!({}),
            });
            for entity in extract_entities(&format!("{} {}", memory.summary, memory.content)) {
                if matches!(entity.kind.as_str(), "file" | "code" | "project") {
                    components.insert(entity.name);
                }
            }
        }

        for component in components.into_iter().take(request.limit.max(10)) {
            let class = if component.ends_with(".rs")
                || component.ends_with(".ts")
                || component.ends_with(".md")
                || component.contains('/')
                || component.contains('\\')
            {
                MapNodeClass::File
            } else {
                MapNodeClass::Component
            };
            let node_id = format!("component:{}", sanitize_id(&component));
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class,
                label: component.clone(),
                detail: Some("architecture surface".to_string()),
                scope: request.workspace.clone(),
                memory_id: None,
                created_at: None,
                updated_at: None,
                importance: Some(0.58),
                confidence: Some(0.72),
                badges: vec!["architecture".to_string()],
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, node_id),
                source: project_node_id.to_string(),
                target: node_id,
                kind: MapEdgeKind::BelongsTo,
                label: Some("component".to_string()),
                weight: 0.6,
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
        }

        self.add_dependency_nodes(assembler, project_node_id, dependencies);
        add_relation_edges(assembler, relations, memories);
    }

    fn add_bug_nodes(
        &self,
        assembler: &mut MapAssembler,
        request: &MapRequest,
        project_node_id: &str,
        memories: &[StoredMemory],
        events: &[TimelineEvent],
        relations: &[MemoryRelation],
    ) {
        for memory in memories
            .iter()
            .filter(|memory| include_memory_in_map(MapType::Bugs, memory))
            .take(request.limit.max(12))
        {
            add_memory_node(assembler, memory, request.why);
            let citation_ids = citations_for_memory(assembler, memory);
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, memory.id),
                source: project_node_id.to_string(),
                target: memory.id.clone(),
                kind: if is_bug_memory(memory) {
                    MapEdgeKind::CausedBy
                } else {
                    MapEdgeKind::FixedBy
                },
                label: Some(if is_bug_memory(memory) {
                    "bug".to_string()
                } else {
                    "fix".to_string()
                }),
                weight: memory.importance.max(0.5),
                citation_ids,
                metadata: json!({}),
            });
        }

        for event in events
            .iter()
            .filter(|event| {
                let body = event.body.to_ascii_lowercase();
                body.contains("patch") || body.contains("fix") || body.contains("bug")
            })
            .take(request.limit.max(8))
        {
            let node_id = format!("bug-event:{}", event.id);
            assembler.add_node(MapNode {
                id: node_id.clone(),
                class: MapNodeClass::Milestone,
                label: event.body.clone(),
                detail: Some(format!("{} timeline event", event.event_type)),
                scope: Some(event.scope.clone()),
                memory_id: event.memory_id.clone(),
                created_at: Some(event.created_at),
                updated_at: Some(event.created_at),
                importance: Some(0.62),
                confidence: Some(0.75),
                badges: vec![event.event_type.clone()],
                citation_ids: Vec::new(),
                metadata: event.data.clone(),
            });
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, node_id),
                source: project_node_id.to_string(),
                target: node_id,
                kind: MapEdgeKind::IntroducedBy,
                label: Some("bug timeline".to_string()),
                weight: 0.62,
                citation_ids: Vec::new(),
                metadata: json!({}),
            });
        }

        add_relation_edges(assembler, relations, memories);
    }

    fn add_dependency_map_nodes(
        &self,
        assembler: &mut MapAssembler,
        request: &MapRequest,
        project_node_id: &str,
        memories: &[StoredMemory],
        dependencies: &[DependencyInfo],
    ) {
        self.add_dependency_nodes(assembler, project_node_id, dependencies);
        for memory in memories
            .iter()
            .filter(|memory| include_memory_in_map(MapType::Dependencies, memory))
            .take(request.limit.max(10))
        {
            add_memory_node(assembler, memory, request.why);
            let citation_ids = citations_for_memory(assembler, memory);
            assembler.add_edge(MapEdge {
                id: format!("edge:{}:{}", project_node_id, memory.id),
                source: project_node_id.to_string(),
                target: memory.id.clone(),
                kind: MapEdgeKind::Mentions,
                label: Some("dependency memory".to_string()),
                weight: memory.importance.max(0.45),
                citation_ids,
                metadata: json!({}),
            });
        }
    }
}

fn render_markdown(map: &MemoryMap) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", map.title));
    out.push_str(&format!(
        "- type: `{}`\n- generated: `{}`\n",
        map.map_type.as_str(),
        map.generated_at.to_rfc3339()
    ));
    if let Some(workspace) = &map.workspace {
        out.push_str(&format!("- workspace: `{workspace}`\n"));
    }
    if let Some(path) = &map.source_path {
        out.push_str(&format!("- source path: `{path}`\n"));
    }
    out.push('\n');

    if !map.notes.is_empty() {
        out.push_str("## Notes\n");
        for note in &map.notes {
            out.push_str(&format!("- {}\n", note));
        }
        out.push('\n');
    }

    if !map.summary.is_empty() {
        out.push_str("## Summary\n");
        for line in &map.summary {
            out.push_str(&format!("- {}\n", line));
        }
        out.push('\n');
    }

    out.push_str("## Nodes\n");
    for node in &map.nodes {
        out.push_str(&format!("- {} [{}]\n", node.label, node.class.as_str()));
        if let Some(detail) = &node.detail {
            out.push_str(&format!("  {}\n", detail));
        }
        if let Some(created_at) = node.created_at {
            out.push_str(&format!("  when: {}\n", created_at.date_naive()));
        }
        if !node.badges.is_empty() {
            out.push_str(&format!("  badges: {}\n", node.badges.join(", ")));
        }
    }
    out.push('\n');

    out.push_str("## Edges\n");
    for edge in &map.edges {
        out.push_str(&format!(
            "- {} -> {} ({})\n",
            edge.source,
            edge.target,
            edge.kind.as_str()
        ));
    }
    out.push('\n');

    if !map.citations.is_empty() {
        out.push_str("## Citations\n");
        for citation in &map.citations {
            let mut location = String::new();
            if let Some(path) = &citation.source_path {
                location.push_str(path);
            }
            if let Some(line) = citation.source_line {
                if !location.is_empty() {
                    location.push(':');
                }
                location.push_str(&line.to_string());
            }
            if location.is_empty() {
                location = citation
                    .source_commit
                    .clone()
                    .unwrap_or_else(|| "memory".to_string());
            }
            out.push_str(&format!("- {} ({})\n", citation.label, location));
        }
    }

    out
}

fn render_mermaid(map: &MemoryMap) -> String {
    let mut out = String::from("flowchart TD\n");
    for node in &map.nodes {
        out.push_str(&format!(
            "  {}[\"{}\"]\n",
            mermaid_id(&node.id),
            escape_mermaid(&node.label)
        ));
    }
    for edge in &map.edges {
        out.push_str(&format!(
            "  {} -->|{}| {}\n",
            mermaid_id(&edge.source),
            edge.kind.as_str(),
            mermaid_id(&edge.target)
        ));
    }
    out.push('\n');
    out.push_str("  classDef milestone fill:#efe3d0,stroke:#6c584c,color:#1f2421;\n");
    out.push_str("  classDef decision fill:#d8f3dc,stroke:#2d6a4f,color:#081c15;\n");
    out.push_str("  classDef bug fill:#ffe5d9,stroke:#9d0208,color:#370617;\n");
    out.push_str("  classDef fix fill:#e0fbfc,stroke:#005f73,color:#001219;\n");
    out.push_str("  classDef file fill:#f5f3ff,stroke:#4338ca,color:#1e1b4b;\n");
    out.push_str("  classDef component fill:#fff3bf,stroke:#bc6c25,color:#4a2c0b;\n");
    out.push_str("  classDef workspace fill:#cfe1b9,stroke:#4f772d,color:#132a13;\n");
    out.push_str("  classDef entity fill:#f1f3f5,stroke:#495057,color:#212529;\n");
    out.push_str("  classDef source fill:#f8f9fa,stroke:#6c757d,color:#343a40;\n");
    out.push_str("  classDef release fill:#dbeafe,stroke:#1d4ed8,color:#1e3a8a;\n");
    for node in &map.nodes {
        out.push_str(&format!(
            "  class {} {};\n",
            mermaid_id(&node.id),
            node.class.as_str()
        ));
    }
    out
}

fn render_html(map: &MemoryMap) -> String {
    let data = serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string());
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <style>
    :root {{
      --bg: #f6f1e8;
      --ink: #1f2421;
      --muted: rgba(31,36,33,0.7);
      --panel: rgba(255,255,255,0.88);
      --line: #d8cdb8;
      --accent: #146356;
      --mono: "IBM Plex Mono", Consolas, monospace;
      --sans: "Space Grotesk", "Segoe UI", sans-serif;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      font-family: var(--sans);
      color: var(--ink);
      background:
        radial-gradient(circle at top right, rgba(20,99,86,0.16), transparent 30%),
        linear-gradient(180deg, #f8f3eb 0%, #efe7da 100%);
    }}
    main {{ max-width: 1280px; margin: 0 auto; padding: 24px; }}
    h1 {{ margin: 0 0 8px; font-size: clamp(2rem, 4vw, 3.6rem); }}
    .muted {{ color: var(--muted); }}
    .hero {{ display: grid; gap: 12px; padding-bottom: 20px; }}
    .filters {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 12px;
      padding: 18px;
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 18px;
      margin-bottom: 18px;
      backdrop-filter: blur(8px);
    }}
    input, select {{
      width: 100%;
      padding: 10px 12px;
      border-radius: 12px;
      border: 1px solid var(--line);
      font: inherit;
      background: white;
    }}
    .grid {{
      display: grid;
      grid-template-columns: 2fr 1fr;
      gap: 18px;
    }}
    .panel {{
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 20px;
      padding: 18px;
      backdrop-filter: blur(8px);
    }}
    .cards {{
      display: grid;
      gap: 12px;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    }}
    .card {{
      border: 1px solid var(--line);
      border-radius: 18px;
      padding: 14px;
      background: rgba(255,255,255,0.92);
    }}
    .card h3 {{ margin: 0 0 8px; font-size: 1.05rem; }}
    .badges {{ display: flex; flex-wrap: wrap; gap: 8px; margin: 10px 0; }}
    .badge {{
      border-radius: 999px;
      background: rgba(20,99,86,0.1);
      border: 1px solid rgba(20,99,86,0.2);
      padding: 4px 8px;
      font-size: 0.8rem;
    }}
    details {{ border-top: 1px solid var(--line); padding-top: 14px; margin-top: 14px; }}
    .edge-list, .citation-list, .notes-list {{
      list-style: none; padding: 0; margin: 0; display: grid; gap: 10px;
    }}
    .edge-list li, .citation-list li, .notes-list li {{
      padding: 10px 12px;
      border: 1px solid var(--line);
      border-radius: 14px;
      background: rgba(255,255,255,0.86);
      font-size: 0.95rem;
    }}
    .mono {{ font-family: var(--mono); }}
    @media (max-width: 900px) {{
      .grid {{ grid-template-columns: 1fr; }}
    }}
  </style>
</head>
<body>
  <main>
    <section class="hero">
      <h1>{title}</h1>
      <p class="muted">Static local-first map export from memory.cpp. Search nodes, filter by class, and follow citations back to the source of truth.</p>
    </section>
    <section class="filters">
      <label><span class="muted">Search</span><input id="search" placeholder="Find node, file, bug, decision..." /></label>
      <label><span class="muted">Class</span><select id="classFilter"><option value="">All classes</option></select></label>
      <label><span class="muted">Date from</span><input id="fromFilter" type="date" /></label>
      <label><span class="muted">Date to</span><input id="toFilter" type="date" /></label>
    </section>
    <section class="grid">
      <section class="panel">
        <h2>Map Nodes</h2>
        <div id="cards" class="cards"></div>
      </section>
      <section class="panel">
        <h2>Map Notes</h2>
        <ul id="notes" class="notes-list"></ul>
        <details open>
          <summary>Edges</summary>
          <ul id="edges" class="edge-list"></ul>
        </details>
        <details open>
          <summary>Citations</summary>
          <ul id="citations" class="citation-list"></ul>
        </details>
      </section>
    </section>
  </main>
  <script>
    const map = {data};
    const cards = document.getElementById('cards');
    const edges = document.getElementById('edges');
    const citations = document.getElementById('citations');
    const notes = document.getElementById('notes');
    const search = document.getElementById('search');
    const classFilter = document.getElementById('classFilter');
    const fromFilter = document.getElementById('fromFilter');
    const toFilter = document.getElementById('toFilter');

    for (const note of map.notes || []) {{
      const item = document.createElement('li');
      item.textContent = note;
      notes.appendChild(item);
    }}

    const classes = [...new Set((map.nodes || []).map(node => node.class))].sort();
    for (const cls of classes) {{
      const option = document.createElement('option');
      option.value = cls;
      option.textContent = cls;
      classFilter.appendChild(option);
    }}

    function render() {{
      cards.innerHTML = '';
      edges.innerHTML = '';
      citations.innerHTML = '';
      const query = search.value.trim().toLowerCase();
      const classValue = classFilter.value;
      const from = fromFilter.value || null;
      const to = toFilter.value || null;
      const visibleNodes = (map.nodes || []).filter(node => {{
        const haystack = [node.label, node.detail || '', ...(node.badges || [])].join(' ').toLowerCase();
        if (query && !haystack.includes(query)) return false;
        if (classValue && node.class !== classValue) return false;
        const stamp = (node.created_at || node.updated_at || '').slice(0, 10);
        if (from && stamp && stamp < from) return false;
        if (to && stamp && stamp > to) return false;
        return true;
      }});
      const visibleIds = new Set(visibleNodes.map(node => node.id));
      for (const node of visibleNodes) {{
        const card = document.createElement('article');
        card.className = 'card';
        const when = node.created_at ? node.created_at.slice(0, 10) : '';
        const badges = (node.badges || []).map(tag => `<span class="badge">${{tag}}</span>`).join('');
        const citationsHtml = (node.citation_ids || []).map(id => {{
          const citation = (map.citations || []).find(entry => entry.id === id);
          if (!citation) return '';
          const href = citation.source_path ? `file:///${{citation.source_path.replace(/\\\\/g, '/')}}` : '#';
          const label = citation.source_line ? `${{citation.label}}:${{citation.source_line}}` : citation.label;
          return `<li><a href="${{href}}">${{label}}</a></li>`;
        }}).join('');
        card.innerHTML = `
          <h3>${{node.label}}</h3>
          <div class="muted">${{node.class}}${{when ? ' · ' + when : ''}}</div>
          <div class="badges">${{badges}}</div>
          <p>${{node.detail || ''}}</p>
          <details>
            <summary>Sources</summary>
            <ul>${{citationsHtml || '<li>No citations recorded.</li>'}}</ul>
          </details>
        `;
        cards.appendChild(card);
      }}

      for (const edge of (map.edges || []).filter(edge => visibleIds.has(edge.source) || visibleIds.has(edge.target))) {{
        const item = document.createElement('li');
        item.innerHTML = `<span class="mono">${{edge.source}}</span> → <span class="mono">${{edge.target}}</span> <span class="muted">(${{edge.kind}})</span>`;
        edges.appendChild(item);
      }}

      for (const citation of map.citations || []) {{
        const item = document.createElement('li');
        const href = citation.source_path ? `file:///${{citation.source_path.replace(/\\\\/g, '/')}}` : '#';
        const label = citation.source_line ? `${{citation.label}}:${{citation.source_line}}` : citation.label;
        item.innerHTML = citation.source_path
          ? `<a href="${{href}}">${{label}}</a>`
          : `<span>${{label}}</span>`;
        citations.appendChild(item);
      }}
    }}

    [search, classFilter, fromFilter, toFilter].forEach(element => element.addEventListener('input', render));
    render();
  </script>
</body>
</html>"#,
        title = escape_html(&map.title),
        data = data
    )
}

fn add_memory_node(assembler: &mut MapAssembler, memory: &StoredMemory, why: bool) {
    let detail = if why {
        extract_reason(memory).or_else(|| Some(memory.summary.clone()))
    } else {
        Some(memory.summary.clone())
    };
    let citation_ids = citations_for_memory(assembler, memory);
    let mut badges = vec![
        memory.kind.as_str().to_string(),
        format!("confidence {:.2}", memory.attributes.confidence),
    ];
    if memory.attributes.human_confirmed {
        badges.push("human-confirmed".to_string());
    }
    if matches!(memory.attributes.status, crate::MemoryStatus::Superseded) {
        badges.push("superseded".to_string());
    }

    assembler.summary_line(format!(
        "{} [{}] in {}",
        memory.summary,
        classify_memory(memory).as_str(),
        memory.scope
    ));
    assembler.add_node(MapNode {
        id: memory.id.clone(),
        class: classify_memory(memory),
        label: memory.summary.clone(),
        detail,
        scope: Some(memory.scope.clone()),
        memory_id: Some(memory.id.clone()),
        created_at: Some(memory.created_at),
        updated_at: Some(memory.updated_at),
        importance: Some(memory.importance),
        confidence: Some(memory.attributes.confidence),
        badges,
        citation_ids,
        metadata: json!({
            "kind": memory.kind,
            "status": memory.attributes.status,
            "permission": memory.attributes.permission,
            "derived": memory.derived,
        }),
    });
}

fn citations_for_memory(assembler: &mut MapAssembler, memory: &StoredMemory) -> Vec<String> {
    let Some(source) = &memory.attributes.source else {
        return Vec::new();
    };

    let path = source
        .source_file
        .clone()
        .or_else(|| source.source.clone())
        .or_else(|| source.source_app.clone());
    let citation_id = format!("citation:memory:{}", sanitize_id(&memory.id));
    assembler.add_citation(MapCitation {
        id: citation_id.clone(),
        label: memory.summary.clone(),
        memory_id: Some(memory.id.clone()),
        source_path: path,
        source_line: source.source_line,
        source_commit: source.source_commit.clone(),
        source_conversation_id: source.source_conversation_id.clone(),
        source_app: source.source_app.clone(),
        snippet: Some(summarize_text(&memory.content, 180)),
    });
    vec![citation_id]
}

fn add_relation_edges(
    assembler: &mut MapAssembler,
    relations: &[MemoryRelation],
    memories: &[StoredMemory],
) {
    let memory_ids = memories
        .iter()
        .map(|memory| memory.id.as_str())
        .collect::<HashSet<_>>();
    for relation in relations {
        if !memory_ids.contains(relation.source_memory_id.as_str())
            && !memory_ids.contains(relation.target_memory_id.as_str())
        {
            continue;
        }
        assembler.add_edge(MapEdge {
            id: relation.id.clone(),
            source: relation.source_memory_id.clone(),
            target: relation.target_memory_id.clone(),
            kind: map_relation_kind(&relation.relation),
            label: Some(relation.relation.clone()),
            weight: relation.weight,
            citation_ids: Vec::new(),
            metadata: relation.data.clone(),
        });
    }
}

fn add_chronological_edges(assembler: &mut MapAssembler) {
    let mut timeline_nodes = assembler
        .nodes
        .values()
        .filter(|node| node.created_at.is_some())
        .cloned()
        .collect::<Vec<_>>();
    timeline_nodes.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.label.cmp(&right.label))
    });
    for pair in timeline_nodes.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let edge_id = format!("edge:chrono:{}:{}", left.id, right.id);
        assembler.add_edge(MapEdge {
            id: edge_id,
            source: left.id.clone(),
            target: right.id.clone(),
            kind: MapEdgeKind::IntroducedBy,
            label: Some("chronological".to_string()),
            weight: 0.5,
            citation_ids: Vec::new(),
            metadata: json!({ "chronological": true }),
        });
    }
}

fn filter_to_impact(assembler: &mut MapAssembler, target: &str) {
    let target = target.to_ascii_lowercase();
    let Some(root_id) = assembler
        .nodes
        .values()
        .find(|node| {
            node.id.eq_ignore_ascii_case(&target)
                || node.label.to_ascii_lowercase().contains(&target)
        })
        .map(|node| node.id.clone())
    else {
        return;
    };

    let mut adjacency = HashMap::<String, Vec<String>>::new();
    for edge in assembler.edges.values() {
        adjacency
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        adjacency
            .entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }

    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(root_id.clone(), 0usize)]);
    while let Some((node_id, depth)) = queue.pop_front() {
        if !seen.insert(node_id.clone()) || depth > 2 {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&node_id) {
            for neighbor in neighbors {
                queue.push_back((neighbor.clone(), depth + 1));
            }
        }
    }

    assembler.nodes.retain(|id, _| seen.contains(id));
    assembler
        .edges
        .retain(|_, edge| seen.contains(&edge.source) && seen.contains(&edge.target));
    let kept_citations = assembler
        .nodes
        .values()
        .flat_map(|node| node.citation_ids.iter().cloned())
        .collect::<HashSet<_>>();
    assembler
        .citations
        .retain(|id, _| kept_citations.contains(id));
}

fn nodes_equivalent(left: &MapNode, right: &MapNode) -> bool {
    left.class == right.class
        && left.label == right.label
        && left.detail == right.detail
        && left.scope == right.scope
        && left.memory_id == right.memory_id
        && left.badges == right.badges
        && left.citation_ids == right.citation_ids
}

fn include_memory_in_map(map_type: MapType, memory: &StoredMemory) -> bool {
    match map_type {
        MapType::Evolution | MapType::Timeline => {
            memory.importance >= 0.45
                || matches!(
                    memory.kind,
                    MemoryKind::Decision
                        | MemoryKind::Bug
                        | MemoryKind::Summary
                        | MemoryKind::Workflow
                )
        }
        MapType::Decisions => is_decision_memory(memory),
        MapType::Architecture => {
            is_decision_memory(memory)
                || memory
                    .attributes
                    .tags
                    .iter()
                    .any(|tag| matches!(tag.as_str(), "architecture" | "component" | "design"))
                || !related_file_entities(memory).is_empty()
        }
        MapType::Bugs => is_bug_memory(memory) || is_fix_memory(memory),
        MapType::Dependencies => {
            memory
                .attributes
                .tags
                .iter()
                .any(|tag| matches!(tag.as_str(), "dependency" | "deps" | "package"))
                || memory.content.to_ascii_lowercase().contains("dependency")
        }
    }
}

fn classify_memory(memory: &StoredMemory) -> MapNodeClass {
    if is_decision_memory(memory) {
        MapNodeClass::Decision
    } else if is_bug_memory(memory) {
        MapNodeClass::Bug
    } else if is_fix_memory(memory) {
        MapNodeClass::Fix
    } else if related_file_entities(memory)
        .iter()
        .any(|entity| entity.contains('/') || entity.contains('\\') || entity.ends_with(".rs"))
    {
        MapNodeClass::File
    } else {
        MapNodeClass::Milestone
    }
}

fn is_decision_memory(memory: &StoredMemory) -> bool {
    matches!(memory.kind, MemoryKind::Decision)
        || memory.summary.to_ascii_lowercase().contains("decision")
        || memory.content.to_ascii_lowercase().contains("because")
}

fn is_bug_memory(memory: &StoredMemory) -> bool {
    matches!(memory.kind, MemoryKind::Bug)
        || memory
            .attributes
            .tags
            .iter()
            .any(|tag| matches!(tag.as_str(), "bug" | "incident" | "regression" | "error"))
        || {
            let lower = memory.summary.to_ascii_lowercase();
            lower.contains("bug") || lower.contains("error") || lower.contains("failure")
        }
}

fn is_fix_memory(memory: &StoredMemory) -> bool {
    memory
        .attributes
        .tags
        .iter()
        .any(|tag| matches!(tag.as_str(), "fix" | "patch" | "resolution"))
        || {
            let lower = memory.summary.to_ascii_lowercase();
            lower.contains("fix")
                || lower.contains("patch")
                || lower.contains("resolved")
                || lower.contains("mitigation")
        }
}

fn related_file_entities(memory: &StoredMemory) -> Vec<String> {
    let mut entities = extract_entities(&format!("{} {}", memory.summary, memory.content))
        .into_iter()
        .filter(|entity| {
            let kind = entity.kind.as_str();
            kind == "file" || kind == "code" || kind == "project"
        })
        .map(|entity| entity.name)
        .collect::<Vec<_>>();

    if let Some(source) = &memory.attributes.source {
        if let Some(file) = source.source_file.clone().or_else(|| source.source.clone()) {
            entities.push(file);
        }
    }

    entities.sort();
    entities.dedup();
    entities
}

fn extract_reason(memory: &StoredMemory) -> Option<String> {
    if let Some(reason) = memory
        .metadata
        .get("reason")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
    {
        return Some(reason);
    }

    let text = memory.content.trim();
    if let Some((_, reason)) = text.split_once("because") {
        return Some(format!("Reason: {}", summarize_text(reason.trim(), 180)));
    }
    if let Some((head, _)) = text.split_once('.') {
        return Some(summarize_text(head.trim(), 180));
    }

    if !text.is_empty() {
        return Some(summarize_text(text, 180));
    }

    None
}

fn map_relation_kind(value: &str) -> MapEdgeKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "depends_on" | "dependency" => MapEdgeKind::DependsOn,
        "fixed_by" | "patch" | "resolved_by" => MapEdgeKind::FixedBy,
        "supersedes" | "changed_to" => MapEdgeKind::Supersedes,
        "contradicts" => MapEdgeKind::Contradicts,
        "mentions" => MapEdgeKind::Mentions,
        "belongs_to" => MapEdgeKind::BelongsTo,
        "caused_by" => MapEdgeKind::CausedBy,
        _ => MapEdgeKind::IntroducedBy,
    }
}

fn scan_docs(path: &Path, limit: usize) -> Result<Vec<DocInfo>> {
    let mut files = Vec::new();
    collect_doc_files(path, &mut files, limit.max(1) * 4)?;
    let mut docs = Vec::new();
    for file in files.into_iter().take(limit.max(1) * 2) {
        if let Ok(contents) = fs::read_to_string(&file) {
            let title = contents
                .lines()
                .find_map(|line| line.trim().strip_prefix('#'))
                .map(|line| line.trim().to_string())
                .unwrap_or_else(|| {
                    file.file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("document")
                        .to_string()
                });
            let summary = contents
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| summarize_text(line, 200))
                .unwrap_or_else(|| "source document".to_string());
            docs.push(DocInfo {
                path: file.display().to_string(),
                title,
                summary,
            });
        }
    }
    Ok(docs)
}

fn collect_doc_files(path: &Path, files: &mut Vec<PathBuf>, limit: usize) -> Result<()> {
    if files.len() >= limit {
        return Ok(());
    }

    if path.is_file() {
        if is_doc_file(path) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if matches!(name, ".git" | "target" | "node_modules" | ".next" | "dist") {
                continue;
            }
            collect_doc_files(&path, files, limit)?;
        } else if is_doc_file(&path) {
            files.push(path);
        }
        if files.len() >= limit {
            break;
        }
    }

    Ok(())
}

fn is_doc_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        Some(ext) if matches!(ext.as_str(), "md" | "markdown" | "txt" | "rst")
    )
}

fn scan_git(path: &Path, limit: usize) -> Result<GitInfo> {
    if !path.exists() {
        return Ok(GitInfo {
            notes: vec![
                "git enrichment unavailable because source path does not exist".to_string(),
            ],
            ..Default::default()
        });
    }

    let root = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };

    let count = ProcessCommand::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "rev-list",
            "--count",
            "HEAD",
        ])
        .output();
    let Ok(count) = count else {
        return Ok(GitInfo {
            notes: vec![
                "git enrichment unavailable because git is missing or the path is not a repository"
                    .to_string(),
            ],
            ..Default::default()
        });
    };
    if !count.status.success() {
        return Ok(GitInfo {
            notes: vec!["git enrichment unavailable because the repository has no commits".into()],
            ..Default::default()
        });
    }

    let commit_count = String::from_utf8_lossy(&count.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(0);
    if commit_count == 0 {
        return Ok(GitInfo {
            notes: vec!["git enrichment unavailable because the repository has no commits".into()],
            ..Default::default()
        });
    }

    let log = ProcessCommand::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "log",
            "--date=iso-strict",
            &format!("-n{}", limit.max(1)),
            "--pretty=format:%H|%ad|%s",
        ])
        .output()?;
    let mut commits = Vec::new();
    if log.status.success() {
        for line in String::from_utf8_lossy(&log.stdout).lines() {
            let mut parts = line.splitn(3, '|');
            let sha = parts.next().unwrap_or_default().to_string();
            let timestamp = parts.next().unwrap_or_default();
            let title = parts.next().unwrap_or_default().to_string();
            let created_at = DateTime::parse_from_rfc3339(timestamp)
                .ok()
                .map(|value| value.with_timezone(&Utc));
            if !sha.is_empty() && !title.is_empty() {
                commits.push(GitCommit {
                    sha,
                    title,
                    created_at,
                });
            }
        }
    }

    let tag_output = ProcessCommand::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "tag",
            "--sort=-creatordate",
        ])
        .output();
    let releases = tag_output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .take(8)
                .map(|line| GitRelease {
                    name: line.trim().to_string(),
                    commit: None,
                })
                .filter(|release| !release.name.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(GitInfo {
        notes: vec![format!("git enrichment added {} commits", commits.len())],
        commits,
        releases,
    })
}

fn scan_dependencies(path: &Path) -> Result<Vec<DependencyInfo>> {
    let root = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };
    let mut dependencies = Vec::new();

    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.exists() {
        let contents = fs::read_to_string(cargo_toml)?;
        let mut in_dependencies = false;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_dependencies =
                    trimmed == "[dependencies]" || trimmed == "[workspace.dependencies]";
                continue;
            }
            if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((name, _)) = trimmed.split_once('=') {
                dependencies.push(DependencyInfo {
                    ecosystem: "cargo".to_string(),
                    name: name.trim().to_string(),
                });
            }
        }
    }

    let package_json = root.join("package.json");
    if package_json.exists() {
        let value: Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
        for key in ["dependencies", "devDependencies"] {
            if let Some(object) = value.get(key).and_then(Value::as_object) {
                for name in object.keys() {
                    dependencies.push(DependencyInfo {
                        ecosystem: "npm".to_string(),
                        name: name.clone(),
                    });
                }
            }
        }
    }

    dependencies.sort_by(|left, right| {
        left.ecosystem
            .cmp(&right.ecosystem)
            .then_with(|| left.name.cmp(&right.name))
    });
    dependencies
        .dedup_by(|left, right| left.ecosystem == right.ecosystem && left.name == right.name);
    Ok(dependencies)
}

fn infer_project_name(path: Option<&Path>) -> Option<String> {
    path.and_then(|path| {
        if path.is_dir() {
            path.file_name()
        } else {
            path.parent().and_then(Path::file_name)
        }
    })
    .and_then(|value| value.to_str())
    .map(|value| value.to_string())
}

fn memory_in_range(
    memory: &StoredMemory,
    from: Option<&DateTime<Utc>>,
    to: Option<&DateTime<Utc>>,
) -> bool {
    let created_at = memory.created_at;
    from.is_none_or(|value| created_at >= *value) && to.is_none_or(|value| created_at <= *value)
}

fn event_in_range(
    event: &TimelineEvent,
    from: Option<&DateTime<Utc>>,
    to: Option<&DateTime<Utc>>,
) -> bool {
    from.is_none_or(|value| event.created_at >= *value)
        && to.is_none_or(|value| event.created_at <= *value)
}

fn summarize_text(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let short = text
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{short}…")
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn mermaid_id(value: &str) -> String {
    let sanitized = sanitize_id(value);
    if sanitized.is_empty() {
        "node".to_string()
    } else {
        format!("n_{}", sanitized)
    }
}

fn escape_mermaid(value: &str) -> String {
    value.replace('"', "'")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
