# memory.cpp

![CI](https://github.com/KirtiRamchandani/memory.cpp/actions/workflows/ci.yml/badge.svg)
![Release](https://github.com/KirtiRamchandani/memory.cpp/actions/workflows/release.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)

[Quick start](docs/quickstart.md) / [Install](docs/install.md) / [CLI](docs/cli.md) / [AI context](docs/context-packs.md) / [Context compiler](docs/context-compiler.md) / [Inference bottlenecks](docs/inference-bottlenecks.md) / [Maps](docs/maps.md) / [Safety](docs/safety.md) / [Roadmap](docs/roadmap.md)

**Your repo remembers.**

`memory.cpp` helps your repo remember what happened, why it changed, what broke, how it was fixed, what to do next, and what AI coding tools should know - locally, safely, and simply.

Remember more. Send less. Run faster.

Stop paying AI to reread the same context. `memory.cpp` compiles local repo memory into smaller, safer task packs so AI tools get the right context without stale decisions, duplicated logs, or accidental secrets.

## Hot topics

- **Public developer adoption release:** daily briefs, AI context packs, project maps, PR summaries, handoff bundles, and shareable Markdown/HTML artifacts.
- **Single-page product site:** open [website/index.html](website/index.html) for the aligned website hub with links to docs, examples, recipes, launch assets, and workflows.
- **Local-first trust model:** SQLite storage under `.memory.cpp/`, read-only MCP by default, candidate review before uncertain memory, and no cloud account required.
- **Context compiler:** `memory compile`, `memory token-firewall`, `memory cache-plan`, and `memory kv-report` help reduce duplicated prompt context before it reaches an AI assistant.
- **CI hardening:** Linux, macOS, and Windows run format, Clippy, tests, and smoke checks. Release builds generate checksums.
- **Honest maturity:** local memory, context compilation, pack generation, safety checks, and static reports are implemented as local-first CLI/API surfaces. Optional editor/MCP attachment remains review-gated and never uploads data by default.

---

## Quick start

Install or preview the installer:

```bash
./scripts/install.sh --dry-run
./scripts/install.sh
```

PowerShell:

```powershell
./scripts/install.ps1 -DryRun
./scripts/install.ps1
```

Start a repo memory workspace:

```bash
memory setup --developer --yes
memory dev morning
memory context write --for cursor --output .memory.cpp/context/cursor.md
memory map --type evolution --output html --save .memory.cpp/maps/evolution.html
memory doctor
```

If you are working from source:

```bash
cargo build -p memory-cli
cargo run -p memory-cli -- setup --developer --yes
cargo run -p memory-cli -- dev morning
```

## What memory.cpp does

The main goal of `memory.cpp` is to make a software repository explainable and resumable with minimal setup.

- Captures durable project memory: decisions, fixes, commands, TODOs, risks, and roadmap notes.
- Turns Git, docs, terminal, CI, and manual notes into reviewable memory candidates.
- Generates daily developer summaries such as `memory dev morning`, `memory today`, and `memory next`.
- Builds AI assistant context packs for Cursor, Codex, Claude, Continue, VS Code, Ollama, and generic tools.
- Creates project evolution maps, timelines, why/impact views, and static shareable artifacts.
- Keeps storage local by default in SQLite, with `.memoryignore`, redaction, and approval gates.

## What it is not

- Not a hosted SaaS memory service.
- Not a replacement for Git history.
- Not a vector database you have to design around.
- Not a team sync platform, billing system, plugin marketplace, mobile pack, fuzzing suite, or AppSec platform.
- Not a tool that uploads repo data by default.

## Core commands

| Command | Why developers run it |
| --- | --- |
| `memory wow` | Run the local wow loop and write a shareable report, pack, doctor JSON, bench JSON, and dashboard. |
| `memory autopilot "fix checkout bug" --for codex` | Compile context, write a provider pack, audit cache layout, and print exact next commands. |
| `memory ship-demo` | Generate deterministic offline demo artifacts for launch or README screenshots. |
| `memory setup --developer --yes` | Create safe local defaults for a repo. |
| `memory dev morning` | See what changed, what broke, and what to do next. |
| `memory dev resume` | Reconstruct interrupted work. |
| `memory dev context --for cursor` | Print an AI-ready context block. |
| `memory context write --for codex` | Write a context pack to disk. |
| `memory compile "fix checkout bug" --provider openai --budget 1500` | Compile a smaller task prompt from local repo memory. |
| `memory prefill-report "fix checkout bug"` | Estimate prompt prefill work avoided before generation. |
| `memory kv-budget "fix checkout bug" --max-kv-tokens 4096` | Compile context under a KV-aware budget. |
| `memory signal-density "fix checkout bug"` | Show useful signal versus duplicate, stale, or noisy prompt material. |
| `memory token-firewall "fix checkout bug"` | Show duplicate, stale, noisy, or unsafe context blocked before prompting. |
| `memory batch-plan --file requests.json --provider openai` | Group requests by shared stable prefixes and fresh suffixes. |
| `memory cache-plan "answer support ticket" --provider claude` | Print a provider-aware stable-prefix/fresh-suffix layout plan. |
| `memory cache-audit --file prompt.md --provider openai` | Detect why provider cache hits may fail. |
| `memory cache-hash` | Print a stable-prefix hash for cache planning. |
| `memory cache-stability` | Explain whether the latest stable prefix is likely cacheable. |
| `memory kv-report "summarize customer history"` | Estimate prompt-side KV pressure avoided by sending less context. |
| `memory runtime-plan "fix checkout bug" --runtime llama.cpp` | Print vendor-neutral runtime hints for local engines. |
| `memory trace compress --file agent-log.txt` | Turn a noisy tool trace into a small reusable summary. |
| `memory trace-rollup --from agent-log.json --every 50` | Roll long-running agent sessions into compact state. |
| `memory mistake "Use pnpm only. Never npm."` | Store a local mistake-firewall rule for future context packs. |
| `memory explain-compile "fix checkout bug"` | Show why context was included or excluded. |
| `memory memories list` | Inspect the local memory vault. |
| `memory profile update "Prefers short PR summaries"` | Build local user/app/agent profile memory. |
| `memory ask "what broke last time billing changed?"` | Answer from local memory with evidence IDs. |
| `memory warnings "change auth flow"` | Show stale memories, secrets, and prior risks for a task. |
| `memory flight start --goal "fix checkout bug" --tool codex` | Start a local AI work-session recorder. |
| `memory context-diff latest previous` | Diff generated context packs like code. |
| `memory agents-score --for codex` | Score how ready the repo is for AI coding tools. |
| `memory trust-report` | Audit active, stale, low-confidence, and evidence-free memories. |
| `memory mcp-scan` | Inspect local MCP config for risky write tools or secret-like wording. |
| `memory mcp-harden --dry-run` | Preview a read-only MCP policy for safe attachment. |
| `memory sign` | Write local hash metadata for generated packs and reports. |
| `memory verify` | Verify generated artifacts against the local manifest. |
| `memory roi --input-cost 2.50` | Estimate approximate token-cost avoidance from local savings logs. |
| `memory leaderboard` | Show top token-waste sources, reused memories, stale blocks, and cache prefixes. |
| `memory map --type evolution --output html` | Generate a project evolution map. |
| `memory map why "SQLite storage"` | Explain why a node or decision exists. |
| `memory inbox review` | Approve, edit, or reject candidate memories. |
| `memory git watch --once --dry-run` | Preview Git-derived memory candidates. |
| `memory terminal search "how did I run tests?"` | Recall useful command history after opt-in. |
| `memory ci explain-failure` | Summarize imported CI failure logs. |
| `memory pr summary --base main` | Generate a PR-ready change summary. |
| `memory pr-comment --base main` | Generate Markdown suitable for a PR comment. |
| `memory pr-context --base main` | Generate reviewer context from branch changes and memory. |
| `memory git-learn --since HEAD~5` | Convert recent Git activity into reviewable local memory candidates. |
| `memory branch-summary --base main` | Print or write a branch summary without network calls. |
| `memory handoff new-dev` | Create a private-safe onboarding bundle. |
| `memory share status` | Create a shareable repo memory summary. |
| `memory release-check` | Check release readiness from the CLI. |

## Daily workflow

```bash
memory dev morning
memory inbox review
memory dev context --for cursor
memory dev next
```

What just happened: `memory.cpp` summarized the repo state, surfaced pending memory candidates, generated AI context, and suggested practical next commands.

## AI coding workflow

```bash
memory attach cursor --dry-run
memory attach claude --dry-run
memory context write --for generic --budget 4000 --output .memory.cpp/context/generic.md
```

MCP integrations are read-only by default. Write-capable memory tools stay disabled or approval-gated unless you explicitly opt in.

## Context compiler and token firewall

```bash
memory mistake "Use pnpm only. Never npm."
memory compile "fix checkout bug" --provider openai --budget 1500
memory prefill-report "fix checkout bug"
memory kv-budget "fix checkout bug" --max-kv-tokens 4096
memory signal-density "fix checkout bug"
memory cache-plan "fix checkout bug" --provider claude
memory cache-audit --file tests/fixtures/inference/provider_cache_bad_order.md --provider openai
memory batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
memory kv-report "fix checkout bug"
memory runtime-profile list
memory runtime-plan "fix checkout bug" --runtime llama.cpp
memory trace compress --file examples/agent-log.txt
memory trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
memory doctor "add CSV export" --provider gemini
memory demo multi-model
```

What just happened: `memory.cpp` selected high-signal local memory, excluded stale or duplicated context, redacted secret-like material, and printed a provider-aware prompt layout.

Important honesty note: `memory.cpp` does not directly compress a provider's KV cache. It reduces prompt size and estimated KV pressure by preventing unnecessary tokens from entering the model. Runtime KV quantization, prefix caching, batching, and speculative decoding remain separate runtime/provider features.

## How memory.cpp addresses the 8 inference bottlenecks

1. Prefill cost: compile smaller prompts with `memory compile` and estimate avoided prompt processing with `memory prefill-report`.
2. KV cache memory: reduce tokens entering the model with `memory kv-budget` and estimate KV pressure avoided with `memory kv-report`.
3. Attention over long context: increase signal density with `memory signal-density` by excluding duplicate, stale, and low-relevance context.
4. Tool/result/history bloat: compress long traces into state using `memory trace compress` and `memory trace-rollup`.
5. Batching effects: group shared stable prefixes and dynamic suffixes with `memory batch-plan`.
6. Speculative decoding: produce runtime hints for local engines with `memory runtime-plan` while leaving kernel-level behavior to the serving runtime.
7. Cache hits/misses: plan stable prefixes with `memory cache-plan` and detect cache-breaking prompt order with `memory cache-audit`.
8. Model architecture/serving engine: use `memory runtime-profile list` and runtime-specific plans for llama.cpp, Ollama, vLLM, SGLang, and generic providers.

`memory doctor "<task>" --provider openai` now includes an `Inference Cost Stack` with raw context tokens, compiled context tokens, fresh suffix tokens, cacheable prefix tokens, omitted tokens, estimated prefill reduction, estimated KV positions avoided, signal density, duplicate/stale tokens blocked, compressed tool trace tokens, provider cache strategy, and runtime strategy.

See [Inference bottlenecks](docs/inference-bottlenecks.md) for fixtures, command examples, and the exact output shapes.

## Local context control plane

The linked product goal is bigger than one feature: `memory.cpp` should feel like a local-first control plane for AI context. This release now exposes deterministic, local command surfaces for the major layers without adding cloud accounts, telemetry, or fragile new storage systems.

```bash
memory memories list
memory profile show --scope user
memory explain-compile "fix checkout bug" --provider openai
memory trust-report
memory flight start --goal "fix checkout bug" --tool codex
memory ask "what broke last time checkout changed?"
memory warnings "change auth flow"
memory context-diff latest previous
memory ingest file README.md
memory shared-context export
memory heatmap --html
memory agents-score --for codex
memory badge --for codex
memory recipe list
memory roi --input-cost 2.50
memory docs search "context compiler"
memory examples run billing-export
```

What just happened: memory.cpp kept the data local, reused the existing memories/events/workspaces/inbox/provenance model, and added inspectable reports for memory vaults, profile memory, pack explainability, cache stability, trust, flight recording, context tests, proactive warnings, shared context, static reports, agent readiness, recipes, and approximate ROI.

### Offline demo paths

```bash
memory demo multi-model
memory wow
memory autopilot "fix billing export bug" --for codex --budget 1500
memory ship-demo
memory pack "fix checkout bug" --for gemini --budget 1500
memory pack "fix checkout bug" --for mcp --budget 1500
memory attach gemini --dry-run
memory attach mcp --dry-run
memory mcp-scan
memory mcp-harden --dry-run
memory sign
memory verify
memory docs list
memory docs summarize
memory docs search "provider cache"
memory examples list
memory examples run coding-agent
```

What just happened: the demo creates provider-specific packs, doctor JSON, benchmark JSON, and an HTML report under `.memory.cpp/demo/` without network calls or accounts.

## Project maps and time machine

```bash
memory map --type evolution --output html --save .memory.cpp/maps/evolution.html
memory timeline week --output .memory.cpp/share/repo-timeline.md
memory rewind last-week
memory changed --since 2026-05-01
```

Maps and timelines are static artifacts you can inspect locally, commit intentionally, or share after review.

## Shareable artifacts

```bash
memory share status --output .memory.cpp/share/project-memory-summary.md
memory share map --output .memory.cpp/share/project-evolution-map.html
memory share onboarding --output .memory.cpp/share/onboarding-brief.md
memory pr comment --base main --output .memory.cpp/share/pr-comment.md
memory handoff new-dev --output .memory.cpp/handoff
```

Artifacts are private-safe by default and are generated from local memory, Git, terminal, CI, maps, and redaction rules when available.

## Maturity matrix

| Surface | Stability | Notes |
| --- | --- | --- |
| SQLite storage | Stable | Durable local memory database. |
| Workspaces | Stable | Repo/project-scoped memory. |
| Remember, search, explain | Stable | Core memory loop. |
| Edit, restore, history | Stable | Versioned memory edits. |
| C API | Beta | Useful for embedding, still pre-1.0. |
| `memory dev morning/resume/next` | Beta | Daily developer workflows. |
| `memory map` HTML/Markdown/Mermaid/JSON | Beta | Signature visualization surface. |
| Candidate inbox | Beta | Review uncertain automatic memory. |
| Git memory and watch | Beta | Local Git-derived candidates. |
| AI context packs | Beta | Practical, cited context for assistants. |
| Terminal memory | Experimental | Opt-in command recording. |
| CI memory | Experimental | Lightweight log ingestion, not a CI platform. |
| FastEmbed/ONNX provider label | Experimental | Current implementation is lightweight local semantic hashing; no bundled ONNX Runtime is claimed. |
| Dashboard/static site | Experimental | Helpful local UI, not a hosted app. |

See [API stability](docs/api-stability.md) and [compatibility](docs/compatibility.md).

## Mental model

```mermaid
flowchart LR
  Repo[Repo] --> Signals[Git, docs, terminal, CI, notes]
  Signals --> Inbox[Candidate inbox]
  Inbox --> Memories[(Local SQLite memories)]
  Memories --> Context[AI context packs]
  Memories --> Maps[Project maps]
  Memories --> Daily[Morning, resume, next]
  Memories --> Share[PR, handoff, release artifacts]
```

## Install paths

| Path | Status | Notes |
| --- | --- | --- |
| Release binary | Preferred | Install scripts try GitHub release assets first. |
| Cargo from source | Stable | `cargo build -p memory-cli` or installer fallback. |
| Linux/macOS shell script | Beta | `scripts/install.sh`, supports `--dry-run`. |
| Windows PowerShell script | Beta | `scripts/install.ps1`, supports `-DryRun`. |
| Homebrew/NPM/Docker | Not shipped | Documented as future packaging, not claimed today. |

## Documentation

Start here:

- [Quickstart](docs/quickstart.md)
- [Install](docs/install.md)
- [First five minutes](docs/first-five-minutes.md)
- [Core concepts](docs/core-concepts.md)
- [CLI reference](docs/cli.md)
- [Developer workflow](docs/dev-workflow.md)
- [AI context packs](docs/context-packs.md)
- [Context compiler and token firewall](docs/context-compiler.md)
- [Inference bottlenecks](docs/inference-bottlenecks.md)
- [Providers](docs/providers.md)
- [Advanced usage](docs/advanced.md)
- [API surface](docs/api.md)
- [Competitive positioning](docs/competitive-positioning.md)
- [Demo script](docs/demo-script.md)
- [Integrations](docs/integrations/cursor.md)
- [Maps](docs/maps.md)
- [Shareable artifacts](docs/share.md)
- [PR workflow](docs/pr-workflow.md)
- [Timeline and rewind](docs/timeline.md)
- [Handoff bundles](docs/handoff.md)
- [Release hardening](docs/release-hardening.md)
- [API stability](docs/api-stability.md)
- [Compatibility](docs/compatibility.md)
- [Performance](docs/performance.md)
- [Optimize AI context recipe](docs/recipes/optimize-ai-context.md)
- [Avoid repeat AI mistakes recipe](docs/recipes/avoid-repeat-ai-mistakes.md)
- [Known limitations](docs/limitations.md)
- [Security policy](SECURITY.md)

## Examples

Useful static examples live under [examples/](examples/):

- [Developer morning](examples/dev-morning.md)
- [AI context for Cursor](examples/cursor-context.md)
- [Context compiler](examples/context-compiler.md)
- [Token firewall](examples/token-firewall.md)
- [KV pressure report](examples/kv-report.md)
- [Tool trace compression](examples/trace-compression.md)
- [Mistake firewall](examples/mistake-firewall.md)
- [Project map HTML](examples/project-map.html)
- [PR comment](examples/pr-comment.md)
- [New-developer handoff](examples/new-dev-handoff.md)
- [Repo timeline](examples/repo-timeline.md)
- [Privacy status](examples/privacy-status.md)

## Validation and release gates

For local development:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p memory-cli
git diff --check
```

All-in-one scripts:

```bash
./scripts/release-candidate.sh
```

PowerShell:

```powershell
./scripts/release-candidate.ps1
```

The GitHub CI matrix runs on Linux, macOS, and Windows.

## Security and privacy

- Data is local by default under `.memory.cpp/`.
- MCP is read-only by default.
- Terminal memory is opt-in.
- Candidate memory is reviewable before approval.
- `.memoryignore` and redaction rules protect sensitive paths and secrets.
- Use `memory privacy status`, `memory redact preview <path>`, and `memory ignore check <path>` before sharing artifacts.

See [SECURITY.md](SECURITY.md), [privacy](docs/privacy.md), [safety](docs/safety.md), and [threat model notes](docs/security.md).

## Community

- [Contributing guide](CONTRIBUTING.md)
- [Launch checklist](docs/launch-checklist.md)
- [Roadmap](docs/roadmap.md)
- [Release process](docs/release-process.md)
- [Dogfooding guide](docs/dogfooding.md)

Good contributions keep the product lane tight: everyday developers, local-first repo memory, AI coding context, maps, safety, and install/docs polish.

## Known limitations

`memory.cpp` is pre-1.0. Some flows are beta or experimental, and local signals only become useful after there is data to summarize. It does not provide hosted sync, team permissions, enterprise policy, or cloud dashboards. See [known limitations](docs/limitations.md) for the full list.
