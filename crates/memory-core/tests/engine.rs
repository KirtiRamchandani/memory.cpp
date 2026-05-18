use std::fs;

use memory_core::{
    evaluate, import_path, EvalCase, ImportFormat, ImportOptions, MapOutputFormat, MapRequest,
    MapType, MemoryEdit, MemoryEngine, MemoryKind, NewMemory, RecallQuery,
};
use tempfile::tempdir;

#[test]
fn remembers_and_recalls_relevant_memory() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(
        NewMemory::new("memory.cpp should feel like SQLite for AI memory: local, tiny, fast.")
            .scope("project")
            .kind("fact")
            .importance(0.95),
    )?;

    let hits = engine.recall(
        RecallQuery::new("What should memory.cpp feel like?")
            .scope("project")
            .limit(3),
    )?;

    assert_eq!(hits.len(), 1);
    assert!(hits[0].memory.summary.contains("SQLite"));
    Ok(())
}

#[test]
fn scope_filter_keeps_projects_separate() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(NewMemory::new("Rust core with a C API layer.").scope("memory.cpp"))?;
    engine.remember(NewMemory::new("Tiny agent runtime under 200MB RAM.").scope("tiny-agent"))?;

    let hits = engine.recall(
        RecallQuery::new("What runtime should be tiny?")
            .scope("tiny-agent")
            .limit(5),
    )?;

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].memory.scope, "tiny-agent");
    Ok(())
}

#[test]
fn compact_scope_creates_summary_memory() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(NewMemory::new("Users prefer one-command local tools.").scope("product"))?;
    engine.remember(
        NewMemory::new("The README must make developers excited quickly.").scope("product"),
    )?;

    let compacted = engine.compact_scope("product", 20)?;

    assert_eq!(compacted.scope, "product");
    assert_eq!(compacted.kind.as_str(), "summary");
    assert!(compacted.content.contains("Compacted long-term memory"));
    Ok(())
}

#[test]
fn context_builder_respects_empty_results() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    let context = engine.context(RecallQuery::new("nothing here").limit(3), 128)?;

    assert!(context.text.contains("Relevant long-term memory"));
    assert!(context.memories.is_empty());
    Ok(())
}

#[test]
fn indexes_entities_for_graph_lookup() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(
        NewMemory::new("memory.cpp integrates with Ollama and Cursor through local memory.")
            .scope("project")
            .kind("fact"),
    )?;

    let graph = engine.entity_graph(Some("project"), 20)?;
    assert!(graph
        .entities
        .iter()
        .any(|node| node.entity.name.contains("memory.cpp")));

    let links = engine.related_entity("Ollama", Some("project"), 10)?;
    assert!(!links.is_empty());
    Ok(())
}

#[test]
fn imports_markdown_files() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("notes.md");
    fs::write(
        &file,
        "# Product Notes\nmemory.cpp should make every local AI app remember useful context.",
    )?;

    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;
    let report = import_path(
        &engine,
        &file,
        &ImportOptions {
            scope: "notes".to_string(),
            kind: MemoryKind::Note,
            format: ImportFormat::Markdown,
            chunk_chars: 500,
            recursive: false,
        },
    )?;

    assert_eq!(report.imported, 1);
    let hits = engine.recall(RecallQuery::new("local AI app remember").scope("notes"))?;
    assert_eq!(hits.len(), 1);
    Ok(())
}

#[test]
fn evaluates_recall_cases() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(NewMemory::new(
        "The proxy injects durable memory into chat completions.",
    ))?;

    let report = evaluate(
        &engine,
        &[EvalCase {
            query: "What does the proxy inject?".to_string(),
            expected: "durable memory".to_string(),
            scope: None,
        }],
        5,
    )?;

    assert_eq!(report.hits, 1);
    Ok(())
}

#[test]
fn computes_stats_without_hanging() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(
        NewMemory::new("User prefers Rust and SQLite for local-first developer tooling.")
            .scope("demo")
            .kind("preference")
            .confidence(0.95),
    )?;
    engine.recall(RecallQuery::new("preferred stack").scope("demo").limit(3))?;

    let stats = engine.stats()?;
    assert_eq!(stats.memories, 1);
    assert_eq!(stats.workspaces, 1);
    assert_eq!(stats.embedding_model, "hash");
    assert!(!stats.top_entities.is_empty());
    Ok(())
}

#[test]
fn includes_global_workspace_memories_by_default() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.create_workspace("global-memory", "shared memory", "global", false)?;
    engine.remember(
        NewMemory::new("Global preference: keep the stack local-first with SQLite.")
            .scope("global-memory")
            .kind("preference"),
    )?;

    let with_global = engine.recall(
        RecallQuery::new("what stack preference should we follow?")
            .scope("project-alpha")
            .limit(5),
    )?;
    assert!(!with_global.is_empty());
    assert_eq!(with_global[0].memory.scope, "global-memory");

    let without_global = engine.recall(
        RecallQuery::new("what stack preference should we follow?")
            .scope("project-alpha")
            .include_global(false)
            .limit(5),
    )?;
    assert!(without_global.is_empty());
    Ok(())
}

#[test]
fn records_version_history_for_edit_restore_and_forget() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    let memory = engine.remember(
        NewMemory::new("Project uses Python for the first prototype.")
            .scope("project")
            .kind("fact"),
    )?;
    engine.edit_memory(
        &memory.id,
        MemoryEdit {
            content: Some("Project migrated to Rust for the production engine.".to_string()),
            ..Default::default()
        },
    )?;
    engine.forget(&memory.id, "stale implementation detail")?;
    engine.restore_memory(&memory.id)?;

    let versions = engine.list_versions(&memory.id, 16)?;
    let actions = versions
        .iter()
        .map(|version| version.action.as_str())
        .collect::<Vec<_>>();
    assert!(actions.contains(&"create"));
    assert!(actions.contains(&"edit"));
    assert!(actions.contains(&"status_change"));
    assert!(actions.contains(&"restore"));
    Ok(())
}

#[test]
fn derived_scores_are_computed_and_explainable() -> memory_core::Result<()> {
    let dir = tempdir()?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    let memory = engine.remember(
        NewMemory::new("User prefers Rust, SQLite, and local-first tools.")
            .scope("prefs")
            .kind("preference")
            .confidence(0.93),
    )?;

    assert!((0.0..=1.0).contains(&memory.derived.freshness));
    assert!((0.0..=1.0).contains(&memory.derived.trust));
    assert!((0.0..=1.0).contains(&memory.derived.usefulness));
    assert!(!memory.derived.explanation.is_empty());
    Ok(())
}

#[test]
fn builds_map_without_git_history_and_compares_snapshots() -> memory_core::Result<()> {
    let dir = tempdir()?;
    fs::write(
        dir.path().join("README.md"),
        "# memory.cpp\nSQLite for AI memory.\n",
    )?;
    let engine = MemoryEngine::open_default(dir.path().join("memory.db"))?;

    engine.remember(
        NewMemory::new("Decision: use SQLite because it keeps the memory file portable.")
            .scope("memorycpp")
            .kind("decision"),
    )?;
    engine.save_snapshot("memorycpp", "before-proxy")?;
    engine.remember(
        NewMemory::new("Added Ollama proxy to inject durable memory into chat completions.")
            .scope("memorycpp")
            .kind("fact")
            .importance(0.9),
    )?;
    engine.save_snapshot("memorycpp", "after-proxy")?;

    let request = MapRequest {
        path: Some(dir.path().to_path_buf()),
        project: Some("memory.cpp".to_string()),
        workspace: Some("memorycpp".to_string()),
        map_type: MapType::Evolution,
        output: MapOutputFormat::Json,
        why: true,
        ..Default::default()
    };

    let map = engine.build_map(&request)?;
    assert!(!map.nodes.is_empty());
    assert!(map
        .notes
        .iter()
        .any(|note| note.contains("git enrichment unavailable")));

    let diff = engine.compare_maps(&request, "before-proxy", "after-proxy")?;
    assert!(!diff.added_nodes.is_empty());
    Ok(())
}
