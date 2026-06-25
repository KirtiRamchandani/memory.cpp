//! Cache-aware compiler (v2) CLI commands.
//!
//! Extracted from main.rs to keep the binary's command surface readable.
//! These call private helpers from the crate root via `use super::*`.
#![allow(clippy::too_many_arguments)]

use super::*;
use anyhow::Result;
use chrono::Utc;
use memory_core::{MemoryEngine, MemoryStatus, RecallQuery};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

pub(crate) fn cache_manifest_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let provider = cli_flag_value(rest, "--provider").unwrap_or_else(|| "generic".to_string());
    let budget = option_usize(rest, "--budget", 1500);
    let task_class = cli_flag_value(rest, "--task-class").unwrap_or_else(|| classify_task(&task));
    let expected_reuse = option_usize(rest, "--expected-reuse", 10);
    let report = build_ai_context_report(engine, &task, &provider, budget, None)?;

    let cache_breaker_warnings = stable_prefix_cache_breakers(&report.stable_prefix);
    let manifest = memory_core::CacheManifest::build(memory_core::CacheManifestInput {
        stable_prefix: &report.stable_prefix,
        fresh_suffix: &report.fresh_suffix,
        provider: &provider,
        task_class: &task_class,
        repo_state_hash: &repo_state_hash(),
        stable_prefix_tokens: report.cacheable_prefix_tokens,
        fresh_suffix_tokens: report.fresh_suffix_tokens,
        expected_reuse,
        cache_breaker_warnings,
    });

    if cli_flag(rest, "--json") || cli_flag(rest, "--for-paper") {
        println!("{}", serde_json::to_string_pretty(&manifest.to_json())?);
    } else {
        println!("cache manifest");
        println!("pack_id: {}", manifest.pack_id);
        println!("stable_prefix_hash: {}", manifest.stable_prefix_hash);
        println!("repo_state_hash: {}", manifest.repo_state_hash);
        println!("task_class: {}", manifest.task_class);
        println!("provider: {}", manifest.provider);
        println!(
            "stable_prefix_tokens: {}  fresh_suffix_tokens: {}",
            manifest.stable_prefix_token_count, manifest.fresh_suffix_token_count
        );
        for region in &manifest.cache_regions {
            println!(
                "- region {} [{}..{}] cacheable={}{}",
                region.name,
                region.start_token,
                region.end_token,
                region.cacheable,
                region
                    .ttl_hint
                    .as_ref()
                    .map(|t| format!(" ttl={t}"))
                    .unwrap_or_default()
            );
        }
        if !manifest.cache_breaker_warnings.is_empty() {
            println!("cache-breaker warnings:");
            for warning in &manifest.cache_breaker_warnings {
                println!("  - {warning}");
            }
        }
        println!("strategy: {}", manifest.suggested_provider_strategy);
        println!(
            "estimated repeated tokens avoided: {} (over {} reuses)",
            manifest.estimated_repeated_tokens_avoided, expected_reuse
        );
        println!(
            "estimated KV positions avoided: {}",
            manifest.estimated_kv_positions_avoided
        );
    }
    Ok(())
}

/// Retrieve the candidate memories used by the context compiler for a task.
/// Shared by the IR inspector and other v2 commands so they all see the same
/// retrieval the compiler does.
pub(crate) fn retrieve_candidates(
    engine: &MemoryEngine,
    task: &str,
) -> Result<Vec<memory_core::RetrievedMemory>> {
    let workspace = current_workspace_name(engine)?.unwrap_or_else(|| "default".to_string());
    let query_text =
        format!("{task} decisions failures fixes rules mistakes commands files risks context");
    let candidates = engine.search(
        RecallQuery::new(query_text)
            .workspace(workspace)
            .limit(48)
            .candidate_pool(96)
            .include_content(true)
            .include_inactive(true),
    )?;
    Ok(candidates)
}

/// `memory ir inspect "<task>"` — lower retrieved memory into MemoryIR and show
/// the typed blocks, their cache roles, volatility, and scores.
pub(crate) fn ir_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    // Accept "ir inspect <task>" and "ir <task>".
    let rest: Vec<String> = if rest.first().map(|s| s.as_str()) == Some("inspect") {
        rest[1..].to_vec()
    } else {
        rest.to_vec()
    };
    let task = task_from_rest(&rest, "current task");
    let candidates = retrieve_candidates(engine, &task)?;
    let ir = memory_core::MemoryIr::from_retrieved(&candidates);

    if cli_flag(&rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&ir.to_json())?);
        return Ok(());
    }

    println!("MemoryIR for task: {task}");
    println!(
        "blocks: {}  (stable: {}, fresh: {}, excluded: {})  total ~{} tokens",
        ir.blocks.len(),
        ir.stable_blocks().count(),
        ir.fresh_blocks().count(),
        ir.excluded_blocks().count(),
        ir.total_tokens()
    );
    for block in &ir.blocks {
        let mark = block
            .exclusion_reason
            .as_ref()
            .map(|r| format!(" [excluded: {r}]"))
            .unwrap_or_default();
        println!(
            "- {} {} role={} vol={} ~{}t util={:.2} fresh={:.2} trust={:.2}{}",
            block.id,
            block.kind.as_str(),
            block.cache_role.as_str(),
            block.volatility_score,
            block.token_count,
            block.utility_score,
            block.freshness_score,
            block.trust_score,
            mark,
        );
    }
    Ok(())
}

/// Path to the persisted negative-memory store (.memory.cpp/negative.json).
pub(crate) fn negative_store_path(engine: &MemoryEngine) -> PathBuf {
    engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("negative.json")
}

pub(crate) fn load_negative_store(engine: &MemoryEngine) -> memory_core::NegativeMemoryStore {
    let path = negative_store_path(engine);
    fs::read_to_string(path)
        .ok()
        .map(|raw| memory_core::NegativeMemoryStore::from_json(&raw))
        .unwrap_or_default()
}

pub(crate) fn save_negative_store(
    engine: &MemoryEngine,
    store: &memory_core::NegativeMemoryStore,
) -> Result<()> {
    let path = negative_store_path(engine);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, store.to_json())?;
    Ok(())
}

/// `memory negative <add|list|remove|explain|learn>`.
pub(crate) fn negative_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let sub = rest.first().map(|s| s.as_str()).unwrap_or("list");
    let args = &rest[rest.len().min(1)..];
    match sub {
        "add" => {
            let target = args
                .iter()
                .find(|a| !a.starts_with("--"))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("usage: memory negative add <path-or-memory-id> --reason \"...\" [--kind stale_doc]"))?;
            let reason =
                cli_flag_value(args, "--reason").unwrap_or_else(|| "superseded".to_string());
            let kind = cli_flag_value(args, "--kind")
                .and_then(|k| memory_core::NegativeKind::parse(&k))
                .unwrap_or(memory_core::NegativeKind::StaleDoc);
            let mut store = load_negative_store(engine);
            let entry = memory_core::NegativeMemory::new(&target, kind, &reason, Utc::now());
            let id = entry.id.clone();
            store.add(entry);
            save_negative_store(engine, &store)?;
            println!("added negative memory {id}: {} [{}]", target, kind.as_str());
        }
        "remove" => {
            let id = args
                .iter()
                .find(|a| !a.starts_with("--"))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("usage: memory negative remove <id>"))?;
            let mut store = load_negative_store(engine);
            if store.remove(&id) {
                save_negative_store(engine, &store)?;
                println!("removed {id}");
            } else {
                println!("no negative memory with id {id}");
            }
        }
        "explain" => {
            let task = task_from_rest(args, "current task");
            let store = load_negative_store(engine);
            let candidates = retrieve_candidates(engine, &task)?;
            let mut hits = Vec::new();
            for c in &candidates {
                let path = c
                    .memory
                    .attributes
                    .source
                    .as_ref()
                    .and_then(|s| s.source_file.clone());
                if let Some(entry) = store.matching(path.as_deref(), Some(c.memory.id.as_str())) {
                    hits.push(json!({
                        "memory": c.memory.id,
                        "summary": c.memory.summary,
                        "excluded_by": entry.id,
                        "kind": entry.kind.as_str(),
                        "reason": entry.reason,
                    }));
                }
            }
            if cli_flag(args, "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"excluded": hits}))?
                );
            } else {
                println!("negative-memory exclusions for: {task}");
                if hits.is_empty() {
                    println!("- none of the retrieved memories match negative memory");
                }
                for h in &hits {
                    println!(
                        "- {} excluded by {} [{}]: {}",
                        h["memory"].as_str().unwrap_or(""),
                        h["excluded_by"].as_str().unwrap_or(""),
                        h["kind"].as_str().unwrap_or(""),
                        h["reason"].as_str().unwrap_or("")
                    );
                }
            }
        }
        "learn" => {
            let path = cli_flag_path(args, "--from").ok_or_else(|| {
                anyhow::anyhow!("usage: memory negative learn --from failed-run.json")
            })?;
            let raw = fs::read_to_string(&path)?;
            let value: Value = serde_json::from_str(&raw)?;
            let mut store = load_negative_store(engine);
            let mut learned = 0usize;
            // Learn from a failed run: any files the run flagged as misleading,
            // stale, or generated become negative memory.
            for key in ["misleading_files", "stale_files", "generated_files"] {
                let kind = match key {
                    "misleading_files" => memory_core::NegativeKind::MisleadingTrace,
                    "stale_files" => memory_core::NegativeKind::StaleDoc,
                    _ => memory_core::NegativeKind::GeneratedFile,
                };
                if let Some(arr) = value[key].as_array() {
                    for item in arr {
                        if let Some(target) = item.as_str() {
                            store.add(memory_core::NegativeMemory::new(
                                target,
                                kind,
                                &format!("learned from {}", path.display()),
                                Utc::now(),
                            ));
                            learned += 1;
                        }
                    }
                }
            }
            save_negative_store(engine, &store)?;
            println!(
                "learned {learned} negative memories from {}",
                path.display()
            );
        }
        _ => {
            let store = load_negative_store(engine);
            if cli_flag(args, "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"entries": store.entries.iter().map(|e| json!({
                        "id": e.id, "target": e.target, "kind": e.kind.as_str(), "reason": e.reason,
                    })).collect::<Vec<_>>()})
                    )?
                );
            } else {
                println!("negative memory ({} entries)", store.entries.len());
                for e in &store.entries {
                    println!(
                        "- {} {} [{}]: {}",
                        e.id,
                        e.target,
                        e.kind.as_str(),
                        e.reason
                    );
                }
                if store.entries.is_empty() {
                    println!(
                        "  (none yet — try: memory negative add docs/old.md --reason superseded)"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Build a freshness proof for one stored memory.
pub(crate) fn prove_memory(memory: &memory_core::StoredMemory) -> FreshnessProof {
    let repo_root = resolve_repo_root(&env::current_dir().unwrap_or_default())
        .unwrap_or_else(|| env::current_dir().unwrap_or_default());
    let source = memory.attributes.source.as_ref();
    let resolved_path = source
        .and_then(|s| s.source_file.clone())
        .map(|f| repo_root.join(f));
    let commit = source.and_then(|s| s.source_commit.clone());
    let commit_reachable = commit.as_ref().map(|c| {
        ProcessCommand::new("git")
            .args(["cat-file", "-e", &format!("{c}^{{commit}}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });
    let reliability = source
        .and_then(|s| s.reliability)
        .unwrap_or(memory.derived.source_reliability);
    memory_core::prove_freshness(
        &memory_core::FreshnessInput {
            memory_id: &memory.id,
            source_file: resolved_path.as_deref(),
            source_line: memory
                .attributes
                .source
                .as_ref()
                .and_then(|s| s.source_line),
            source_commit: commit.as_deref(),
            stored_span_hash: None,
            expires_at: memory.attributes.expires_at,
            human_confirmed: memory.attributes.human_confirmed,
            reliability,
            newer_contradiction: memory.attributes.contradiction_count > 0,
            commit_reachable,
        },
        Utc::now(),
    )
}

/// `memory prove [--memory <id>] [--json]`.
pub(crate) fn prove_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let memories = engine.all_memories(None, true)?;
    let selected: Vec<&memory_core::StoredMemory> = match cli_flag_value(rest, "--memory") {
        Some(id) => memories.iter().filter(|m| m.id == id).collect(),
        None => memories
            .iter()
            .take(option_usize(rest, "--limit", 50))
            .collect(),
    };
    let proofs: Vec<FreshnessProof> = selected.iter().map(|m| prove_memory(m)).collect();
    if cli_flag(rest, "--json") {
        let arr: Vec<Value> = proofs.iter().map(|p| p.to_json()).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "proofs": arr }))?
        );
        return Ok(());
    }
    let fresh = proofs.iter().filter(|p| p.is_fresh()).count();
    println!("freshness proofs: {} memories", proofs.len());
    println!(
        "fresh: {fresh}  stale: {}  unverifiable: {}",
        proofs
            .iter()
            .filter(|p| p.verdict == memory_core::FreshnessVerdict::Stale)
            .count(),
        proofs
            .iter()
            .filter(|p| p.verdict == memory_core::FreshnessVerdict::Unverifiable)
            .count(),
    );
    for p in &proofs {
        println!(
            "- {} {} (score {:.2})",
            p.memory_id,
            p.verdict.as_str(),
            p.score
        );
        if p.verdict != memory_core::FreshnessVerdict::Fresh {
            for c in p
                .checks
                .iter()
                .filter(|c| c.status == memory_core::ProofStatus::Fail)
            {
                println!("    fail: {} — {}", c.name, c.detail);
            }
        }
    }
    Ok(())
}

/// `memory audit-stale` — list memories that fail their freshness proof.
pub(crate) fn audit_stale_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let memories = engine.all_memories(None, true)?;
    let stale: Vec<(String, FreshnessProof)> = memories
        .iter()
        .map(|m| (m.summary.clone(), prove_memory(m)))
        .filter(|(_, p)| p.verdict == memory_core::FreshnessVerdict::Stale)
        .collect();
    if cli_flag(rest, "--json") {
        let arr: Vec<Value> = stale
            .iter()
            .map(|(summary, p)| json!({ "summary": summary, "proof": p.to_json() }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "stale": arr }))?
        );
        return Ok(());
    }
    println!("stale memory audit: {} flagged", stale.len());
    for (summary, p) in &stale {
        let reasons: Vec<&str> = p
            .checks
            .iter()
            .filter(|c| c.status == memory_core::ProofStatus::Fail)
            .map(|c| c.name.as_str())
            .collect();
        println!("- {} [{}]: {}", p.memory_id, summary, reasons.join(", "));
    }
    if stale.is_empty() {
        println!("  no stale memories detected");
    }
    Ok(())
}

/// `memory explain-exclusions [<task>]` — show why context was excluded:
/// negative memory, staleness, secrets, prompt injection, and duplicates.
pub(crate) fn explain_exclusions_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let store = load_negative_store(engine);
    let candidates = retrieve_candidates(engine, &task)?;
    let mut excluded = Vec::new();
    for c in &candidates {
        let text = memory_text(&c.memory);
        let path = c
            .memory
            .attributes
            .source
            .as_ref()
            .and_then(|s| s.source_file.clone());
        let reason = if detect_sensitive_reason(&text).is_some() {
            Some("secret-like content".to_string())
        } else if has_prompt_injection_warning(&text) {
            Some("prompt-injection-like content".to_string())
        } else if is_memory_stale_or_blocked(&c.memory) {
            Some("stale/superseded".to_string())
        } else {
            store
                .matching(path.as_deref(), Some(c.memory.id.as_str()))
                .map(|e| format!("negative memory [{}]: {}", e.kind.as_str(), e.reason))
        };
        if let Some(reason) = reason {
            excluded.push(json!({
                "memory": c.memory.id,
                "summary": c.memory.summary,
                "reason": reason,
            }));
        }
    }
    if cli_flag(rest, "--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "excluded": excluded }))?
        );
    } else {
        println!("excluded context for: {task} ({} blocks)", excluded.len());
        for e in &excluded {
            println!(
                "- {} [{}]: {}",
                e["memory"].as_str().unwrap_or(""),
                e["summary"].as_str().unwrap_or(""),
                e["reason"].as_str().unwrap_or("")
            );
        }
        if excluded.is_empty() {
            println!("  no exclusions — all retrieved memory is clean and fresh");
        }
    }
    Ok(())
}

/// `memory tokens [<task>] --provider openai|anthropic|generic` — count tokens
/// of the compiled pack with a provider-tuned estimator, labeling estimates.
pub(crate) fn tokens_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let provider = cli_flag_value(rest, "--provider").unwrap_or_else(|| "generic".to_string());
    let report = build_ai_context_report(engine, &task, &provider, 1500, None)?;
    let tk = memory_core::Tokenizer::for_provider(&provider);
    let compiled = tk.count(&report.compiled_prompt);
    let stable = tk.count(&report.stable_prefix);
    let fresh = tk.count(&report.fresh_suffix);
    let generic = memory_core::Tokenizer::Generic.count(&report.compiled_prompt);
    if cli_flag(rest, "--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": provider,
                "compiled": compiled.to_json(),
                "stable_prefix": stable.to_json(),
                "fresh_suffix": fresh.to_json(),
                "generic_estimate": generic.to_json(),
            }))?
        );
    } else {
        println!("token count for: {task}  (provider: {provider})");
        println!(
            "compiled prompt: ~{} tokens (estimate, {})",
            compiled.tokens,
            if compiled.measured {
                "measured"
            } else {
                "approx"
            }
        );
        println!("  stable prefix: ~{} tokens", stable.tokens);
        println!("  fresh suffix:  ~{} tokens", fresh.tokens);
        println!("generic estimate: ~{} tokens", generic.tokens);
        println!("note: counts are estimates unless a provider tokenizer is enabled.");
    }
    Ok(())
}

/// `memory kv-sim [<task>] --model llama-3.1-8b [--runtime vllm]` — estimate KV
/// bytes for raw vs compiled context. Honest estimate, not a measurement.
pub(crate) fn kv_sim_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let model_name = cli_flag_value(rest, "--model").unwrap_or_else(|| "llama-3.1-8b".to_string());
    let mut profile = match cli_flag_path(rest, "--profile") {
        Some(path) => {
            let raw = fs::read_to_string(path)?;
            serde_json::from_str::<memory_core::ModelProfile>(&raw)?
        }
        None => memory_core::ModelProfile::preset(&model_name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown model '{model_name}'. Try llama-3.1-8b, llama-3.1-70b, mistral-7b, qwen2.5-7b, generic-7b, or pass --profile <file.json>"
            )
        })?,
    };
    if let Some(runtime) = cli_flag_value(rest, "--runtime") {
        profile = profile.with_runtime(&runtime);
    }
    let report = build_ai_context_report(engine, &task, "generic", 1500, None)?;
    let comparison = memory_core::KvComparison::new(
        &profile,
        report.raw_tokens,
        report.compiled_tokens,
        report.cacheable_prefix_tokens,
        report.fresh_suffix_tokens,
    );
    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&comparison.to_json())?);
    } else {
        println!("KV SIMULATION (estimate) — model: {}", comparison.model);
        println!("raw tokens:      {}", comparison.raw_tokens);
        println!("compiled tokens: {}", comparison.compiled_tokens);
        println!(
            "  stable prefix: {}   fresh suffix: {}",
            comparison.stable_prefix_tokens, comparison.fresh_suffix_tokens
        );
        println!(
            "estimated KV raw:      {:.1} MiB",
            memory_core::mib(comparison.estimated_kv_bytes_raw)
        );
        println!(
            "estimated KV compiled: {:.1} MiB",
            memory_core::mib(comparison.estimated_kv_bytes_compiled)
        );
        println!(
            "estimated KV avoided:  {:.1} MiB ({:.1}%)",
            memory_core::mib(comparison.estimated_kv_bytes_avoided),
            comparison.estimated_kv_percent_reduction
        );
        println!("{}", comparison.note);
    }
    Ok(())
}

/// `memory optimize <task> [--objective kv_efficiency] [-O3] [--budget N]
/// [--max-kv-mb M] [--model llama-3.1-8b]` — run the KV-aware optimizer over the
/// task's MemoryIR and show what was selected/excluded and why.
pub(crate) fn optimize_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let budget = option_usize(rest, "--budget", 1500);

    // Resolve objective: -O level wins, then --objective, else minimal_sufficient.
    let opt_level = rest.iter().find_map(|a| memory_core::OptLevel::parse(a));
    let objective = opt_level.map(|l| l.objective()).unwrap_or_else(|| {
        cli_flag_value(rest, "--objective")
            .and_then(|o| memory_core::Objective::parse(&o))
            .unwrap_or(memory_core::Objective::MinimalSufficient)
    });

    let kv_bytes_per_token = cli_flag_value(rest, "--model")
        .and_then(|m| memory_core::ModelProfile::preset(&m))
        .map(|p| p.kv_bytes(1))
        .unwrap_or(0);
    let max_kv_bytes = cli_flag_value(rest, "--max-kv-mb")
        .and_then(|v| v.parse::<u64>().ok())
        .map(|mb| mb * 1024 * 1024);

    let candidates = retrieve_candidates(engine, &task)?;
    let ir = memory_core::MemoryIr::from_retrieved(&candidates);
    let result = memory_core::optimize(
        &ir.blocks,
        &memory_core::OptimizerConfig {
            objective,
            token_budget: budget,
            max_kv_bytes,
            kv_bytes_per_token,
        },
    );

    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&result.to_json())?);
        return Ok(());
    }
    println!("optimize: {task}");
    println!(
        "objective: {}{}  budget: {} tokens  selected: {} tokens",
        result.objective,
        opt_level
            .map(|l| format!(" ({})", l.as_str()))
            .unwrap_or_default(),
        result.token_budget,
        result.selected_tokens
    );
    println!("included ({}):", result.selected.len());
    for b in &result.selected {
        println!(
            "  + {} {} role={} ~{}t score={:.4} — {}",
            b.id, b.kind, b.cache_role, b.tokens, b.score, b.reason
        );
    }
    if !result.excluded.is_empty() {
        println!("excluded ({}):", result.excluded.len());
        for b in &result.excluded {
            println!("  - {} {} ~{}t — {}", b.id, b.kind, b.tokens, b.reason);
        }
    }
    Ok(())
}

/// `memory rehydrate mem://file/src/auth.rs#L10-L50` — expand a handle from disk.
pub(crate) fn rehydrate_command(rest: &[String]) -> Result<()> {
    let uri = rest
        .iter()
        .find(|a| a.starts_with("mem://"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("usage: memory rehydrate mem://file/<path>#L10-L50"))?;
    let handle = memory_core::Handle::parse(&uri)
        .ok_or_else(|| anyhow::anyhow!("could not parse handle: {uri}"))?;
    let root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let result = memory_core::rehydrate_file(&root, &handle);
    if cli_flag(rest, "--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "handle": result.handle,
                "found": result.found,
                "content": result.content,
            }))?
        );
    } else if result.found {
        println!("{}", result.content);
    } else {
        println!("could not rehydrate {}: {}", result.handle, result.content);
    }
    Ok(())
}

/// `memory minimize "<task>" --must-include "a,b"` — find the smallest subset of
/// retrieved memory whose combined text still contains every required string.
pub(crate) fn minimize_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let strategy = memory_core::MinStrategy::parse(
        &cli_flag_value(rest, "--strategy").unwrap_or_else(|| "greedy".to_string()),
    );
    // Oracle: required substrings. Default to salient task words if unspecified.
    let required: Vec<String> = match cli_flag_value(rest, "--must-include") {
        Some(v) => v
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        None => task
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .map(|s| s.to_string())
            .collect(),
    };

    let candidates = retrieve_candidates(engine, &task)?;
    let blocks: Vec<memory_core::MinBlock> = candidates
        .iter()
        .map(|c| memory_core::MinBlock {
            id: c.memory.id.clone(),
            text: memory_text(&c.memory),
            tokens: estimate_tokens(&memory_text(&c.memory)),
        })
        .collect();

    let result = memory_core::minimize(&blocks, strategy, |kept| {
        let joined = kept
            .iter()
            .map(|b| b.text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        required.iter().all(|r| joined.contains(r))
    });

    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&result.to_json())?);
    } else {
        println!("minimal sufficient context for: {task}");
        println!(
            "raw: {} blocks / {} tokens  ->  minimized: {} blocks / {} tokens",
            result.raw_blocks, result.raw_tokens, result.minimized_blocks, result.minimized_tokens
        );
        println!("oracle satisfied: {}", result.oracle_satisfied);
        println!("essential: {}", result.essential_ids.join(", "));
        if !result.removed_ids.is_empty() {
            println!("removed {} blocks", result.removed_ids.len());
        }
    }
    Ok(())
}

/// `memory debt [--json] [--fix-plan] [--badge]` — context debt score.
pub(crate) fn debt_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let memories = engine.all_memories(None, true)?;
    let total = memories.len();
    let stale = memories
        .iter()
        .filter(|m| {
            matches!(
                m.attributes.status,
                MemoryStatus::Superseded | MemoryStatus::Contradicted | MemoryStatus::Archived
            )
        })
        .count();
    let contradictions = memories
        .iter()
        .filter(|m| m.attributes.contradiction_count > 0)
        .count();
    let secrets = memories
        .iter()
        .filter(|m| m.derived.sensitivity > 0.6)
        .count();
    let unverified = memories
        .iter()
        .filter(|m| {
            m.importance > 0.7 && m.attributes.confidence < 0.5 && m.attributes.source.is_none()
        })
        .count();
    let report = memory_core::debt_score(memory_core::DebtFactors {
        stale_memories: stale,
        unresolved_contradictions: contradictions,
        duplicate_memories: 0,
        unverified_high_impact: unverified,
        secret_risk_memories: secrets,
        unstable_prefix_rate: 0.0,
        trace_bloat_blocks: 0,
        outdated_test_recipes: 0,
        total_memories: total,
    });

    if cli_flag(rest, "--badge") {
        println!(
            "![context debt](https://img.shields.io/badge/context_debt-{}%2F100-{})",
            report.score,
            if report.score < 40 {
                "brightgreen"
            } else if report.score < 70 {
                "yellow"
            } else {
                "red"
            }
        );
        return Ok(());
    }
    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&report.to_json())?);
        return Ok(());
    }
    println!("Context Debt: {}/100  ({})", report.score, report.grade);
    if !report.top_causes.is_empty() {
        println!("Main causes:");
        for c in &report.top_causes {
            println!("  - {c}");
        }
    }
    if cli_flag(rest, "--fix-plan") || !report.fix_plan.is_empty() {
        println!("Fix plan:");
        for (i, f) in report.fix_plan.iter().enumerate() {
            println!("  {}. {f}", i + 1);
        }
    }
    Ok(())
}

/// `memory autopsy "<task>" [--missing a,b] [--budget N]` — explain likely
/// context-side failure causes for a task's compiled pack.
pub(crate) fn autopsy_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let budget = option_usize(rest, "--budget", 1500);
    let report = build_ai_context_report(engine, &task, "generic", budget, None)?;
    let candidates = retrieve_candidates(engine, &task)?;
    let ir = memory_core::MemoryIr::from_retrieved(&candidates);
    let missing: Vec<String> = cli_flag_value(rest, "--missing")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let autopsy = memory_core::autopsy_analyze(&memory_core::AutopsyInput {
        blocks: &ir.blocks,
        budget,
        compiled_tokens: report.compiled_tokens,
        missing_essentials: &missing,
    });
    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&autopsy.to_json())?);
    } else {
        println!("autopsy: {task}");
        match &autopsy.likely_failure {
            Some(f) => println!("likely failure: {f}"),
            None => println!("no obvious context-side failure causes found"),
        }
        for f in &autopsy.findings {
            println!("- [{}] {}: {}", f.severity, f.category, f.detail);
            println!("    fix: {}", f.fix);
        }
    }
    Ok(())
}

/// `memory fuzz-context "<task>" [--must-include a,b]` — stress-test a pack.
pub(crate) fn fuzz_context_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let budget = option_usize(rest, "--budget", 1500);
    let required: Vec<String> = cli_flag_value(rest, "--must-include")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let candidates = retrieve_candidates(engine, &task)?;
    let blocks: Vec<memory_core::FuzzBlock> = candidates
        .iter()
        .take(12)
        .map(|c| memory_core::FuzzBlock {
            id: c.memory.id.clone(),
            text: memory_text(&c.memory),
            tokens: estimate_tokens(&memory_text(&c.memory)),
        })
        .collect();
    let report = memory_core::fuzz(&blocks, budget, |kept| {
        let joined = kept
            .iter()
            .map(|b| b.text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let has_required = required.iter().all(|r| joined.contains(r));
        let safe = !joined.contains("ignore previous instructions");
        has_required && safe
    });
    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&report.to_json())?);
    } else {
        println!("context fuzzing: {task}");
        println!(
            "robustness: {:.0}% ({}/{} mutations survived)",
            report.robustness_score * 100.0,
            report.mutations_survived,
            report.mutations_run
        );
        println!("min safe tokens: {}", report.min_safe_tokens);
        if !report.essential_blocks.is_empty() {
            println!("essential blocks: {}", report.essential_blocks.join(", "));
        }
        for f in &report.failure_conditions {
            println!("  ! {f}");
        }
    }
    Ok(())
}

/// `memory poison simulate|benchmark` and `memory write-firewall`.
pub(crate) fn poison_command(rest: &[String]) -> Result<()> {
    let outcomes = memory_core::poison_benchmark();
    let correct = outcomes.iter().filter(|o| o.correct).count();
    if cli_flag(rest, "--json") {
        let arr: Vec<Value> = outcomes
            .iter()
            .map(|o| {
                json!({
                    "scenario": o.scenario,
                    "expected_block": o.expected_block,
                    "blocked": o.blocked,
                    "action": o.action,
                    "correct": o.correct,
                    "reasons": o.reasons,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({ "accuracy": format!("{correct}/{}", outcomes.len()), "scenarios": arr })
            )?
        );
    } else {
        println!("memory poisoning simulator");
        for o in &outcomes {
            println!(
                "- {:<22} expected={} -> {} [{}]",
                o.scenario,
                if o.expected_block { "block" } else { "allow" },
                if o.blocked { "blocked" } else { "allowed" },
                if o.correct { "ok" } else { "MISS" }
            );
        }
        println!("detection accuracy: {correct}/{}", outcomes.len());
    }
    Ok(())
}

/// `memory write-firewall --text "..."` or `--candidate file.json`.
pub(crate) fn write_firewall_command(rest: &[String]) -> Result<()> {
    let candidate = if let Some(path) = cli_flag_path(rest, "--candidate") {
        let raw = fs::read_to_string(path)?;
        serde_json::from_str::<memory_core::WriteCandidate>(&raw)?
    } else {
        memory_core::WriteCandidate {
            text: cli_flag_value(rest, "--text").unwrap_or_default(),
            source: cli_flag_value(rest, "--source"),
            confidence: cli_flag_value(rest, "--confidence")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8),
        }
    };
    let verdict = memory_core::screen_write(&candidate);
    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&verdict.to_json())?);
    } else {
        println!(
            "write firewall: {} ({})",
            if verdict.blocked {
                "BLOCKED"
            } else {
                "allowed"
            },
            verdict.action
        );
        for r in &verdict.reasons {
            println!("  - {r}");
        }
        if let Some(p) = &verdict.recommended_policy {
            println!("policy: {p}");
        }
    }
    Ok(())
}

/// Directory for generated reports/artifacts (.memory.cpp/reports).
pub(crate) fn reports_dir(engine: &MemoryEngine) -> PathBuf {
    engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("reports")
}

/// `memory bench camc|compare [--model M] [--budget N] [--json]`
/// `memory bench report --html` — write an HTML CAMC-Bench report.
pub(crate) fn camc_bench_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let model = cli_flag_value(rest, "--model").unwrap_or_else(|| "llama-3.1-8b".to_string());
    let budget = option_usize(rest, "--budget", 1500);
    let report = memory_core::run_camc_bench(&model, budget);
    let sub = rest.first().map(|s| s.as_str()).unwrap_or("camc");

    if sub == "report" || cli_flag(rest, "--html") {
        let dir = reports_dir(engine);
        fs::create_dir_all(&dir)?;
        let path = dir.join("camc-bench.html");
        fs::write(&path, render_bench_html(&report))?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    if cli_flag(rest, "--json") {
        println!("{}", serde_json::to_string_pretty(&report.to_json())?);
    } else {
        println!("{}", report.to_table());
    }
    Ok(())
}

pub(crate) fn render_bench_html(report: &memory_core::BenchReport) -> String {
    let mut rows = String::new();
    for r in &report.results {
        let highlight = if r.strategy == "full CAMC" {
            " style=\"background:#eef9f0;font-weight:600\""
        } else {
            ""
        };
        rows.push_str(&format!(
            "<tr{highlight}><td>{}</td><td>{}</td><td>{}</td><td>{:.1}%</td><td>{}</td><td>{}</td><td>{}</td><td>{}/{}</td></tr>",
            r.strategy, r.total_tokens, r.cacheable_prefix_tokens, r.token_reduction_vs_raw_percent,
            r.stale_included, r.contradictions_included, r.secret_leakage, r.tasks_passed, r.tasks_total
        ));
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>CAMC-Bench</title>\
<style>body{{font-family:system-ui,sans-serif;margin:2rem;color:#111}}table{{border-collapse:collapse;width:100%}}\
th,td{{border:1px solid #ddd;padding:.5rem .75rem;text-align:right}}th:first-child,td:first-child{{text-align:left}}\
th{{background:#0b1020;color:#fff}}h1{{margin-bottom:.2rem}}.note{{color:#555;font-size:.9rem;margin-top:1rem}}</style></head>\
<body><h1>CAMC-Bench</h1><p>Cache-Aware Memory Compilation for Coding Agents — model {}, budget {}.</p>\
<table><thead><tr><th>Strategy</th><th>Tokens</th><th>Cacheable prefix</th><th>Reduction vs raw</th>\
<th>Stale</th><th>Contradictions</th><th>Secrets</th><th>Task success</th></tr></thead><tbody>{}</tbody></table>\
<p class=\"note\">{}</p></body></html>",
        report.model, report.budget, rows, report.note
    )
}

/// `memory paper run|tables|export|figures|appendix` — generate paper-ready
/// artifacts from CAMC-Bench into .memory.cpp/paper/.
pub(crate) fn paper_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let sub = rest.first().map(|s| s.as_str()).unwrap_or("run");
    let model = cli_flag_value(rest, "--model").unwrap_or_else(|| "llama-3.1-8b".to_string());
    let budget = option_usize(rest, "--budget", 1500);
    let report = memory_core::run_camc_bench(&model, budget);
    let dir = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("paper");

    match sub {
        "tables" => println!("{}", report.to_markdown()),
        "export" => println!("{}", serde_json::to_string_pretty(&report.to_json())?),
        "appendix" => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "benchmark": "CAMC-Bench",
                "corpus": "synthetic bundled fixtures",
                "model": report.model,
                "budget": report.budget,
                "results": report.results.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
            }))?
        ),
        "figures" => {
            fs::create_dir_all(&dir)?;
            let path = dir.join("reduction.svg");
            fs::write(&path, render_reduction_svg(&report))?;
            println!("wrote {}", path.display());
        }
        _ => {
            // run: write all artifacts.
            fs::create_dir_all(&dir)?;
            fs::write(dir.join("tables.md"), report.to_markdown())?;
            fs::write(
                dir.join("report.json"),
                serde_json::to_string_pretty(&report.to_json())?,
            )?;
            fs::write(
                dir.join("methodology.json"),
                serde_json::to_string_pretty(&json!({
                    "benchmark": "CAMC-Bench",
                    "description": "Deterministic comparison of context-selection strategies on a synthetic repo-memory corpus.",
                    "strategies": ["raw full context","grep","BM25","embedding retrieval","CAMC ablations","full CAMC"],
                    "metrics": ["tokens","cacheable_prefix_tokens","token_reduction_vs_raw","stale_included","contradictions_included","secret_leakage","task_success","estimated_kv_mib"],
                    "oracle": "task passes iff all essential memory ids are included and no contradictions are included",
                    "honesty": "Synthetic corpus; estimated KV is prompt-side, not provider KV-cache compression; runtime latency must be measured separately.",
                    "model": report.model,
                    "budget": report.budget,
                }))?,
            )?;
            fs::write(dir.join("reduction.svg"), render_reduction_svg(&report))?;
            fs::write(
                dir.join("reproduce.sh"),
                format!(
                    "#!/usr/bin/env bash\n# Reproduce CAMC-Bench results.\nset -euo pipefail\ncargo run -q -p memory-cli -- bench camc --model {} --budget {} --json\n",
                    report.model, report.budget
                ),
            )?;
            println!("wrote paper artifacts to {}", dir.display());
            println!("- tables.md, report.json, methodology.json, reduction.svg, reproduce.sh");
        }
    }
    Ok(())
}

pub(crate) fn render_reduction_svg(report: &memory_core::BenchReport) -> String {
    let width = 720;
    let bar_h = 26;
    let gap = 12;
    let left = 220;
    let top = 50;
    let height = top + (report.results.len() as i32) * (bar_h + gap) + 40;
    let max_tokens = report
        .results
        .iter()
        .map(|r| r.total_tokens)
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let mut bars = String::new();
    for (i, r) in report.results.iter().enumerate() {
        let y = top + (i as i32) * (bar_h + gap);
        let w = ((r.total_tokens as f32 / max_tokens) * (width - left - 80) as f32) as i32;
        let color = if r.strategy == "full CAMC" {
            "#2e9e5b"
        } else {
            "#5b7cfa"
        };
        bars.push_str(&format!(
            "<text x=\"10\" y=\"{}\" font-size=\"12\" fill=\"#222\">{}</text>\
<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\"/>\
<text x=\"{}\" y=\"{}\" font-size=\"12\" fill=\"#222\">{} tok</text>",
            y + 18,
            r.strategy,
            left,
            y,
            w.max(1),
            bar_h,
            color,
            left + w + 6,
            y + 18,
            r.total_tokens
        ));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\
<style>text{{font-family:system-ui,sans-serif}}</style>\
<text x=\"10\" y=\"24\" font-size=\"16\" font-weight=\"700\">CAMC-Bench: total context tokens by strategy</text>\
{bars}</svg>"
    )
}

pub(crate) fn lock_path(engine: &MemoryEngine) -> PathBuf {
    engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("context.lock")
}

pub(crate) fn ledger_path(engine: &MemoryEngine) -> PathBuf {
    engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("ledger.jsonl")
}

/// Build a deterministic lockfile + provenance ledger entry for a task.
pub(crate) fn build_lock_and_ledger(
    engine: &MemoryEngine,
    task: &str,
    provider: &str,
    budget: usize,
) -> Result<(memory_core::ContextPackLock, memory_core::LedgerEntry)> {
    let report = build_ai_context_report(engine, task, provider, budget, None)?;
    let candidates = retrieve_candidates(engine, task)?;
    let ir = memory_core::MemoryIr::from_retrieved(&candidates);
    let task_class = classify_task(task);
    let repo_state = repo_state_hash();

    // Included = non-excluded blocks; excluded = excluded blocks with reasons.
    let mut included_memories = Vec::new();
    let mut source_hashes = Vec::new();
    let mut excluded_memories = Vec::new();
    for block in &ir.blocks {
        match &block.exclusion_reason {
            Some(reason) => excluded_memories.push(memory_core::ExcludedRecord {
                id: block
                    .source_memory_id
                    .clone()
                    .unwrap_or_else(|| block.id.clone()),
                reason: reason.clone(),
            }),
            None => {
                if let Some(id) = &block.source_memory_id {
                    included_memories.push(id.clone());
                }
                source_hashes.push(block.content_hash.clone());
            }
        }
    }

    // Freshness proofs for the included memories.
    let memories = engine.all_memories(None, true)?;
    let mut proofs_passed = 0usize;
    let mut proofs_failed = 0usize;
    let mut proof_hashes = Vec::new();
    for id in &included_memories {
        if let Some(m) = memories.iter().find(|m| &m.id == id) {
            let proof = prove_memory(m);
            proof_hashes.push(proof.proof_hash.clone());
            if proof.is_fresh() {
                proofs_passed += 1;
            } else {
                proofs_failed += 1;
            }
        }
    }

    let negative_store = load_negative_store(engine);
    let negative_version = memory_core::content_hash("negv", negative_store.to_json().as_bytes());
    let stable_prefix_hash = memory_core::stable_prefix_hash(&report.stable_prefix);
    let fresh_suffix_hash = memory_core::content_hash("suffix", report.fresh_suffix.as_bytes());
    let pack_id = memory_core::pack_id(
        &report.stable_prefix,
        &report.fresh_suffix,
        provider,
        &task_class,
    );
    let pack_hash = memory_core::content_hash("pack", report.compiled_prompt.as_bytes());

    let lock = memory_core::ContextPackLock {
        pack_id: pack_id.clone(),
        compiler_version: memory_core::compiler_version(),
        repo_state_hash: repo_state.clone(),
        task_class: task_class.clone(),
        provider: provider.to_string(),
        memory_ids: included_memories.clone(),
        source_hashes,
        freshness_proof_hashes: proof_hashes,
        negative_memory_version: negative_version,
        stable_prefix_hash: stable_prefix_hash.clone(),
        fresh_suffix_hash: fresh_suffix_hash.clone(),
        token_counts: memory_core::LockTokenCounts {
            stable_prefix: report.cacheable_prefix_tokens,
            fresh_suffix: report.fresh_suffix_tokens,
            compiled: report.compiled_tokens,
        },
    };

    let entry = memory_core::LedgerEntry {
        pack_id,
        recorded_at: Utc::now(),
        provider: provider.to_string(),
        task_class,
        repo_state_hash: repo_state,
        included_memories,
        excluded_memories,
        proofs_passed,
        proofs_failed,
        stable_prefix_hash,
        fresh_suffix_hash,
        stable_prefix_tokens: report.cacheable_prefix_tokens,
        fresh_suffix_tokens: report.fresh_suffix_tokens,
        compiled_tokens: report.compiled_tokens,
        budget,
        cache_regions: vec!["stable_repo_prefix".into(), "fresh_task_suffix".into()],
        generated_files: vec![],
        pack_hash,
    };

    Ok((lock, entry))
}

/// `memory lock [<task>]` writes context.lock and appends a ledger entry.
/// `memory lock diff [<task>]` diffs the current pack against the stored lock.
pub(crate) fn lock_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let is_diff = rest.first().map(|s| s.as_str()) == Some("diff");
    let rest = if is_diff { &rest[1..] } else { rest };
    let task = task_from_rest(rest, "current task");
    let provider = cli_flag_value(rest, "--provider").unwrap_or_else(|| "generic".to_string());
    let budget = option_usize(rest, "--budget", 1500);
    let (lock, entry) = build_lock_and_ledger(engine, &task, &provider, budget)?;

    if is_diff {
        let stored = fs::read_to_string(lock_path(engine))
            .ok()
            .and_then(|raw| memory_core::ContextPackLock::from_json(&raw));
        match stored {
            None => println!("no stored lock — run 'memory lock' first"),
            Some(stored) => {
                let diffs = stored.diff(&lock);
                if diffs.is_empty() {
                    println!("lock matches: pack is reproducible from context.lock");
                } else {
                    println!("lock differs ({} changes):", diffs.len());
                    for d in diffs {
                        println!("  {d}");
                    }
                }
            }
        }
        return Ok(());
    }

    let path = lock_path(engine);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, lock.to_json())?;
    // Append the ledger entry.
    let lpath = ledger_path(engine);
    let mut log = fs::read_to_string(&lpath).unwrap_or_default();
    log.push_str(&entry.to_jsonl());
    log.push('\n');
    fs::write(&lpath, log)?;

    if cli_flag(rest, "--json") {
        println!("{}", lock.to_json());
    } else {
        println!("wrote {}", path.display());
        println!("pack_id: {}", lock.pack_id);
        println!("stable_prefix_hash: {}", lock.stable_prefix_hash);
        println!("lock_hash: {}", lock.lock_hash());
        println!(
            "memories: {}  proofs: {} pass / {} fail",
            lock.memory_ids.len(),
            entry.proofs_passed,
            entry.proofs_failed
        );
    }
    Ok(())
}

/// `memory reproduce [<task>]` — recompute the lock and verify it matches the
/// stored context.lock (proving the pack is deterministically reproducible).
pub(crate) fn reproduce_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let task = task_from_rest(rest, "current task");
    let provider = cli_flag_value(rest, "--provider").unwrap_or_else(|| "generic".to_string());
    let budget = option_usize(rest, "--budget", 1500);
    let (lock, _) = build_lock_and_ledger(engine, &task, &provider, budget)?;
    let stored = fs::read_to_string(lock_path(engine))
        .ok()
        .and_then(|raw| memory_core::ContextPackLock::from_json(&raw));
    match stored {
        None => {
            println!("no context.lock found — run 'memory lock' first");
            Ok(())
        }
        Some(stored) => {
            // The stable prefix is task-independent, so it must always match.
            let prefix_ok = stored.stable_prefix_hash == lock.stable_prefix_hash;
            let full_ok = stored.matches(&lock);
            println!("reproduce: {task}");
            println!(
                "stable prefix reproducible: {}",
                if prefix_ok { "yes" } else { "no" }
            );
            println!(
                "full lock reproducible: {}",
                if full_ok { "yes" } else { "no" }
            );
            if !full_ok {
                for d in stored.diff(&lock) {
                    println!("  {d}");
                }
            }
            Ok(())
        }
    }
}

/// `memory ledger latest|show <pack-id>|export [--json]`.
pub(crate) fn ledger_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let sub = rest.first().map(|s| s.as_str()).unwrap_or("latest");
    let entries = memory_core::LedgerEntry::parse_log(
        &fs::read_to_string(ledger_path(engine)).unwrap_or_default(),
    );
    match sub {
        "export" => {
            let arr: Vec<Value> = entries.iter().map(|e| e.to_json()).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "ledger": arr }))?
            );
        }
        "show" => {
            let id = rest.get(1).cloned().unwrap_or_default();
            match entries.iter().find(|e| e.pack_id == id) {
                Some(e) => println!("{}", serde_json::to_string_pretty(&e.to_json())?),
                None => println!("no ledger entry with pack_id {id}"),
            }
        }
        "diff" => {
            if entries.len() < 2 {
                println!("need at least two ledger entries to diff");
            } else {
                let a = &entries[entries.len() - 2];
                let b = &entries[entries.len() - 1];
                println!("ledger diff: {} -> {}", a.pack_id, b.pack_id);
                println!(
                    "compiled tokens: {} -> {}",
                    a.compiled_tokens, b.compiled_tokens
                );
                let added: Vec<&String> = b
                    .included_memories
                    .iter()
                    .filter(|m| !a.included_memories.contains(m))
                    .collect();
                let removed: Vec<&String> = a
                    .included_memories
                    .iter()
                    .filter(|m| !b.included_memories.contains(m))
                    .collect();
                for m in added {
                    println!("  + {m}");
                }
                for m in removed {
                    println!("  - {m}");
                }
            }
        }
        _ => match entries.last() {
            None => println!("ledger is empty — run 'memory lock' to record a pack"),
            Some(e) => {
                if cli_flag(rest, "--json") {
                    println!("{}", serde_json::to_string_pretty(&e.to_json())?);
                } else {
                    println!("latest ledger entry");
                    println!("pack_id: {}", e.pack_id);
                    println!("provider: {}  task_class: {}", e.provider, e.task_class);
                    println!(
                        "included: {}  excluded: {}  proofs: {}/{}",
                        e.included_memories.len(),
                        e.excluded_memories.len(),
                        e.proofs_passed,
                        e.proofs_passed + e.proofs_failed
                    );
                    println!("compiled tokens: {}", e.compiled_tokens);
                }
            }
        },
    }
    Ok(())
}

/// `memory verify-pack <file>` — recompute the file's content hash and, if a
/// matching ledger entry exists, confirm the pack hash is unchanged.
pub(crate) fn verify_pack_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let path = rest
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("usage: memory verify-pack <pack-file>"))?;
    let content = fs::read_to_string(&path)?;
    let hash = memory_core::content_hash("pack", content.as_bytes());
    let entries = memory_core::LedgerEntry::parse_log(
        &fs::read_to_string(ledger_path(engine)).unwrap_or_default(),
    );
    let matching = entries.iter().find(|e| e.pack_hash == hash);
    println!("pack file: {path}");
    println!("pack_hash: {hash}");
    match matching {
        Some(e) => println!("verified: matches ledger entry {}", e.pack_id),
        None => println!(
            "no ledger entry matches this pack hash (file may have changed or was not locked)"
        ),
    }
    Ok(())
}

/// Coarse task classifier used for cache-manifest task_class labelling.
pub(crate) fn classify_task(task: &str) -> String {
    let lower = task.to_ascii_lowercase();
    if lower.contains("fix") || lower.contains("bug") || lower.contains("error") {
        "bugfix".to_string()
    } else if lower.contains("refactor") || lower.contains("clean") {
        "refactor".to_string()
    } else if lower.contains("test") {
        "testing".to_string()
    } else if lower.contains("review") || lower.contains("audit") {
        "review".to_string()
    } else if lower.contains("add") || lower.contains("implement") || lower.contains("feature") {
        "feature".to_string()
    } else {
        "general".to_string()
    }
}

/// Best-effort repository state hash from git HEAD; falls back to a stable
/// placeholder when git is unavailable so output stays deterministic.
pub(crate) fn repo_state_hash() -> String {
    match ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if sha.is_empty() {
                "git:unknown".to_string()
            } else {
                format!("git:{sha}")
            }
        }
        _ => "git:unknown".to_string(),
    }
}

/// Detect content in a stable prefix that would break prompt caching.
pub(crate) fn stable_prefix_cache_breakers(prefix: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let lower = prefix.to_ascii_lowercase();
    for needle in ["task:", "current error", "tool output", "traceback"] {
        if lower.contains(needle) {
            warnings.push(format!(
                "stable prefix contains volatile marker '{needle}' — move it to the fresh suffix"
            ));
        }
    }
    warnings
}
