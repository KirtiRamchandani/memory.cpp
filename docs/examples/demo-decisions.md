# memory.cpp decisions map

- type: `decisions`
- generated: `2026-05-18T08:44:16.238763700+00:00`
- workspace: `launch-demo`
- source path: `.`

## Notes
- git enrichment unavailable because the repository has no commits

## Summary
- Attach helpers should default to read-only MCP with workspace scoping and credential redaction so the first integration feels trustworthy. [decision] in launch-demo
- memory map evolution should show idea, storage, retrieval, MCP, proxy, attach, and launch polish as a chronological project story. [decision] in launch-demo
- memory dev morning should summarize yesterday's work, open conflicts, recent decisions, recent bugs, and the next recommended action. [decision] in launch-demo
- The viral demo is memory proxy plus memory map evolution: every local chat remembers and every repo can explain itself. [decision] in launch-demo
- Expose memory through MCP so Cursor, Claude, Codex, and VS Code can use memory.cpp without custom integrations. [decision] in launch-demo
- Use SQLite as the core store so memory stays local-first, portable, auditable, and easy to back up. [decision] in launch-demo
- memory.cpp aims to be SQLite for engineering memory: one local memory layer for developers and AI apps. [decision] in launch-demo

## Nodes
- anyhow [component]
  cargo dependency
  badges: cargo, dependency
- blake3 [component]
  cargo dependency
  badges: cargo, dependency
- chrono [component]
  cargo dependency
  badges: cargo, dependency
- clap [component]
  cargo dependency
  badges: cargo, dependency
- criterion [component]
  cargo dependency
  badges: cargo, dependency
- memory.cpp [component]
  derived from .
  badges: project
- rusqlite [component]
  cargo dependency
  badges: cargo, dependency
- serde [component]
  cargo dependency
  badges: cargo, dependency
- serde_json [component]
  cargo dependency
  badges: cargo, dependency
- tempfile [component]
  cargo dependency
  badges: cargo, dependency
- thiserror [component]
  cargo dependency
  badges: cargo, dependency
- tiny_http [component]
  cargo dependency
  badges: cargo, dependency
- ureq [component]
  cargo dependency
  badges: cargo, dependency
- uuid [component]
  cargo dependency
  badges: cargo, dependency
- README.md [file]
  related file or code surface
  badges: related
- crates/memory-cli/src/main.rs [file]
  related file or code surface
  badges: related
- crates/memory-core/src/map.rs [file]
  related file or code surface
  badges: related
- crates/memory-core/src/storage.rs [file]
  related file or code surface
  badges: related
- local-first [file]
  related file or code surface
  badges: related
- memory.cpp [file]
  related file or code surface
  badges: related
- read-only [file]
  related file or code surface
  badges: related
- ADR 0001: SQLite As The Core Store [source]
  Accepted
  badges: doc
- ADR 0002: MCP Read-Only By Default [source]
  Accepted
  badges: doc
- ADR 0003: Map As A First-Class Product Surface [source]
  Accepted
  badges: doc
- Architecture [source]
  `memory.cpp` is designed as a local memory primitive, not just a retrieval helper.
  badges: doc
- C API [source]
  The C ABI is defined in `include/memory_cpp.h` and implemented by `crates/memory-capi`.
  badges: doc
- CLI Reference [source]
  `memory.cpp` ships a small core command tree plus a few launch-polish commands routed through a documented pre-parser.
  badges: doc
- Contributing [source]
  Thanks for helping build `memory.cpp`.
  badges: doc
- Developer Workflow [source]
  The `memory dev` namespace is meant to make the repository feel alive.
  badges: doc
- Integrations [source]
  Start Ollama, then place `memory.cpp` in front of it:
  badges: doc
- MCP [source]
  `memory.cpp` uses MCP as the default integration surface for coding agents.
  badges: doc
- Maps [source]
  `memory map` is the visual proof that memory.cpp is working.
  badges: doc
- memory.cpp decisions map [source]
  - type: `decisions`
  badges: doc
- launch-demo [workspace]
  workspace memory scope
  badges: scope
- memory.cpp aims to be SQLite for engineering memory: one local memory layer for developers and AI apps. [decision]
  memory
  when: 2026-04-30
  badges: decision, confidence 0.97
- Use SQLite as the core store so memory stays local-first, portable, auditable, and easy to back up. [decision]
  Use SQLite as the core store so memory stays local-first, portable, auditable, and easy to back up
  when: 2026-05-01
  badges: decision, confidence 0.96
- Expose memory through MCP so Cursor, Claude, Codex, and VS Code can use memory.cpp without custom integrations. [decision]
  Expose memory through MCP so Cursor, Claude, Codex, and VS Code can use memory
  when: 2026-05-05
  badges: decision, confidence 0.95
- The viral demo is memory proxy plus memory map evolution: every local chat remembers and every repo can explain itself. [decision]
  The viral demo is memory proxy plus memory map evolution: every local chat remembers and every repo can explain itself
  when: 2026-05-07
  badges: decision, confidence 0.94
- memory dev morning should summarize yesterday's work, open conflicts, recent decisions, recent bugs, and the next recommended action. [decision]
  memory dev morning should summarize yesterday's work, open conflicts, recent decisions, recent bugs, and the next recommended action
  when: 2026-05-17
  badges: workflow, confidence 0.94
- memory map evolution should show idea, storage, retrieval, MCP, proxy, attach, and launch polish as a chronological project story. [decision]
  memory map evolution should show idea, storage, retrieval, MCP, proxy, attach, and launch polish as a chronological project story
  when: 2026-05-17
  badges: decision, confidence 0.95
- Attach helpers should default to read-only MCP with workspace scoping and credential redaction so the first integration feels trustworthy. [decision]
  Attach helpers should default to read-only MCP with workspace scoping and credential redaction so the first integration feels trustworthy
  when: 2026-05-18
  badges: decision, confidence 0.92

## Edges
- 09d945d8-e7d1-44cb-9632-ec39aa068861 -> file:crates_memory_core_src_storage_rs (mentions)
- 09d945d8-e7d1-44cb-9632-ec39aa068861 -> file:local_first (mentions)
- 436b6db7-b032-4cbd-a45e-4edd264378b6 -> file:crates_memory_cli_src_main_rs (mentions)
- 480c4857-0e76-452b-9e20-d0d4aabe487c -> file:crates_memory_cli_src_main_rs (mentions)
- 480c4857-0e76-452b-9e20-d0d4aabe487c -> file:read_only (mentions)
- 4995ca54-2b61-4c68-a075-9b1d54269d20 -> file:readme_md (mentions)
- 82609b79-34eb-412d-9a01-8bc0f59ae5a6 -> file:crates_memory_core_src_map_rs (mentions)
- 8fb74677-4c0e-4e02-b997-789b87bfa47f -> file:memory_cpp (mentions)
- 8fb74677-4c0e-4e02-b997-789b87bfa47f -> file:readme_md (mentions)
- ec337dd5-894d-4aba-bfb6-1f3c6238cf46 -> file:crates_memory_cli_src_main_rs (mentions)
- ec337dd5-894d-4aba-bfb6-1f3c6238cf46 -> file:memory_cpp (mentions)
- project:memory_cpp -> 09d945d8-e7d1-44cb-9632-ec39aa068861 (introduced_by)
- project:memory_cpp -> 436b6db7-b032-4cbd-a45e-4edd264378b6 (introduced_by)
- project:memory_cpp -> 480c4857-0e76-452b-9e20-d0d4aabe487c (introduced_by)
- project:memory_cpp -> 4995ca54-2b61-4c68-a075-9b1d54269d20 (introduced_by)
- project:memory_cpp -> 82609b79-34eb-412d-9a01-8bc0f59ae5a6 (introduced_by)
- project:memory_cpp -> 8fb74677-4c0e-4e02-b997-789b87bfa47f (introduced_by)
- project:memory_cpp -> dep:cargo:anyhow (depends_on)
- project:memory_cpp -> dep:cargo:blake3 (depends_on)
- project:memory_cpp -> dep:cargo:chrono (depends_on)
- project:memory_cpp -> dep:cargo:clap (depends_on)
- project:memory_cpp -> dep:cargo:criterion (depends_on)
- project:memory_cpp -> dep:cargo:rusqlite (depends_on)
- project:memory_cpp -> dep:cargo:serde (depends_on)
- project:memory_cpp -> dep:cargo:serde_json (depends_on)
- project:memory_cpp -> dep:cargo:tempfile (depends_on)
- project:memory_cpp -> dep:cargo:thiserror (depends_on)
- project:memory_cpp -> dep:cargo:tiny_http (depends_on)
- project:memory_cpp -> dep:cargo:ureq (depends_on)
- project:memory_cpp -> dep:cargo:uuid (depends_on)
- project:memory_cpp -> ec337dd5-894d-4aba-bfb6-1f3c6238cf46 (introduced_by)
- source:contributing_md -> project:memory_cpp (belongs_to)
- source:docs_adr_0001_sqlite_local_first_md -> project:memory_cpp (belongs_to)
- source:docs_adr_0002_mcp_read_only_default_md -> project:memory_cpp (belongs_to)
- source:docs_adr_0003_map_as_product_surface_md -> project:memory_cpp (belongs_to)
- source:docs_architecture_md -> project:memory_cpp (belongs_to)
- source:docs_c_api_md -> project:memory_cpp (belongs_to)
- source:docs_cli_md -> project:memory_cpp (belongs_to)
- source:docs_dev_workflow_md -> project:memory_cpp (belongs_to)
- source:docs_integrations_md -> project:memory_cpp (belongs_to)
- source:docs_maps_md -> project:memory_cpp (belongs_to)
- source:docs_mcp_md -> project:memory_cpp (belongs_to)
- source:memory_cpp_demo_decisions_md -> project:memory_cpp (belongs_to)
- workspace:launch_demo -> project:memory_cpp (belongs_to)

## Citations
- ADR 0001: SQLite As The Core Store (.\docs\adr\0001-sqlite-local-first.md)
- ADR 0002: MCP Read-Only By Default (.\docs\adr\0002-mcp-read-only-default.md)
- ADR 0003: Map As A First-Class Product Surface (.\docs\adr\0003-map-as-product-surface.md)
- Architecture (./docs/architecture.md)
- Attach helpers should default to read-only MCP with workspace scoping and credential redaction so the first integration feels trustworthy. (crates/memory-cli/src/main.rs)
- C API (.\docs\C_API.md)
- CLI Reference (.\docs\cli.md)
- Contributing (.\CONTRIBUTING.md)
- Developer Workflow (.\docs\dev-workflow.md)
- Expose memory through MCP so Cursor, Claude, Codex, and VS Code can use memory.cpp without custom integrations. (crates/memory-cli/src/main.rs)
- Integrations (.\docs\INTEGRATIONS.md)
- MCP (.\docs\mcp.md)
- Maps (.\docs\maps.md)
- The viral demo is memory proxy plus memory map evolution: every local chat remembers and every repo can explain itself. (README.md)
- Use SQLite as the core store so memory stays local-first, portable, auditable, and easy to back up. (crates/memory-core/src/storage.rs)
- memory dev morning should summarize yesterday's work, open conflicts, recent decisions, recent bugs, and the next recommended action. (crates/memory-cli/src/main.rs)
- memory map evolution should show idea, storage, retrieval, MCP, proxy, attach, and launch polish as a chronological project story. (crates/memory-core/src/map.rs)
- memory.cpp aims to be SQLite for engineering memory: one local memory layer for developers and AI apps. (README.md)
- memory.cpp decisions map (.\.memory.cpp\demo\decisions.md)
