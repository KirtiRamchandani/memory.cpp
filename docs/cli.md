# CLI Reference

`memory.cpp` ships a practical command line for one job: helping your repo remember what happened, why it changed, and what to do next.

## Beginner-friendly commands

These are the first commands to try.

```bash
memory welcome
memory setup --interactive
memory what
memory where
memory today
memory yesterday
memory next
memory show-map
memory show-context
memory show-inbox
memory privacy status
```

## Core commands

```bash
memory init
memory remember
memory recall
memory explain
memory forget
memory patch
memory context
memory compile
memory import
memory watch
memory timeline
memory graph
memory stats
memory list
memory export
memory workspace
memory policy
memory snapshot
memory diff
memory inbox
memory attach
memory serve
memory dashboard
memory proxy
memory mcp
```

## Launch-polish commands

These are routed through a small pre-parser so the launch build avoids a known Clap stack-overflow edge case from an oversized nested command tree.

```bash
memory edit
memory restore
memory demo
memory audit-log
memory doctor
memory dev
memory extract
memory git
memory ignore
memory map
memory start
memory stop
memory status
memory setup
memory tutorial
memory terminal
memory ci
memory embeddings
memory privacy
memory pack
memory token-firewall
memory firewall
memory cache-plan
memory kv-report
memory kv-compile
memory prefill-report
memory kv-budget
memory signal-density
memory batch-plan
memory runtime-profile
memory cache-audit
memory trace-rollup
memory trace
memory mistake
memory mistakes
memory conflicts
memory stale
memory resolve
memory savings
memory runtime-plan
memory bench-context
```

## Most important workflows

### Friendly first run

```bash
memory welcome
memory setup --developer
memory what
memory where
memory privacy status
```

### Remember and recall

```bash
memory remember "Use SQLite for local-first durability." --workspace demo --kind decision
memory recall "why SQLite" --workspace demo --content
memory explain "why SQLite" --workspace demo
```

### Daily development flow

```bash
memory dev morning --workspace demo
memory dev resume "proxy launch" --workspace demo
memory dev explain-repo . --workspace demo
memory dev next --workspace demo
memory dev context --for cursor --workspace demo
```

### Git-aware extraction

```bash
memory git summary --since 14d
memory git decisions --since 30d
memory git bugs --since 30d
memory git ingest --workspace demo --since 14d
memory git watch --once --workspace demo
```

### Candidate inbox

```bash
memory inbox list --workspace demo
memory inbox stats --workspace demo
memory inbox explain <id>
memory inbox edit <id> "Better wording" --kind decision
memory inbox approve-all --confidence-above 0.9
```

### Maps

```bash
memory map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
memory map why "MCP integration" --workspace demo --output markdown
memory map impact "SQLite storage" --workspace demo --output markdown
memory show-map --workspace demo
```

### Terminal memory

```bash
memory terminal enable
memory terminal record "cargo test" --exit-code 0 --duration-ms 12000
memory terminal last-error
memory terminal search "how did I run tests?"
```

Terminal memory is opt-in.

### AI context packs

```bash
memory dev context --for cursor --workspace demo
memory dev context --for codex --workspace demo
memory dev context --for claude --workspace demo
```

### Context compiler and token firewall

```bash
memory compile "fix checkout bug" --provider openai --budget 1500
memory token-firewall "fix checkout bug" --provider openai --budget 2000
memory cache-plan "answer support ticket" --provider claude
memory kv-report "summarize customer history"
memory prefill-report "fix checkout bug"
memory kv-budget "fix checkout bug" --max-kv-tokens 4096
memory signal-density "fix checkout bug"
memory batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
memory runtime-profile list
memory runtime-plan "fix checkout bug" --runtime llama.cpp
memory cache-audit --file tests/fixtures/inference/provider_cache_bad_order.md --provider openai
memory trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
memory trace compress --file examples/agent-log.txt
memory mistake "Use pnpm only. Never npm."
memory doctor "add CSV export" --provider gemini
memory pack "fix checkout bug" --for codex --budget 1500
memory savings
```

These commands are local-first prompt hygiene tools. They compile smaller task context, report estimated prompt-side KV pressure avoided, improve signal density, group shared prefixes, and keep stale or risky memory out of AI prompts. They do not directly control provider KV caches or low-level serving kernels.

`memory doctor "<task>" --provider <provider>` includes an `Inference Cost Stack` section with raw and compiled context tokens, fresh suffix tokens, cacheable prefix tokens, omitted tokens, estimated prefill reduction, KV positions avoided, signal density, duplicate/stale/tool-trace tokens blocked, provider cache strategy, and runtime strategy.

### Embeddings

```bash
memory embeddings status
memory embeddings list
memory embeddings set fastembed
memory embeddings migrate --to fastembed --dry-run
```

`fastembed` in this launch build is a zero-dependency local semantic-hashing backend. Real ONNX Runtime integration is documented as a later backend.

### Runtime management

```bash
memory start --workspace demo --proxy
memory status
memory stop
memory audit-log --limit 20
```

## Output guidance

- Use `--json` for automation.
- Use `--save <path>` with maps and reports you want to keep.
- Use `doctor` before sharing a demo or attaching agents.
- Create `.memoryignore` before importing or watching a real repository.
- Use `proxy --learn --approval-required` when you want passive learning without unattended direct writes.

## Virality and adoption loop

Shareable artifacts:

```bash
memory share status
memory share map --output .memory.cpp/share/project-evolution-map.html
memory share context --private-safe
```

Repo documentation:

```bash
memory docs generate --dry-run
memory docs generate --apply
```

PR and handoff workflows:

```bash
memory pr summary --base main
memory pr checklist --output .memory.cpp/share/pr-checklist.md
memory handoff new-dev --output .memory.cpp/handoff
```

Repo time machine and local activation:

```bash
memory timeline week
memory rewind last-week
memory changed --since "7 days ago"
memory adoption checklist
memory release-check
```
