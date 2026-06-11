# Launch plan

## Target users

- Developers using Codex, Cursor, Claude, Continue, and local models
- Teams that want repo-native memory without cloud accounts
- Security-conscious engineers who need inspectable local context packs

## One-sentence pitch

memory.cpp is a local-first context control plane that turns repo decisions, failures, and tool traces into smaller verified packs for AI coding tools.

## 60-second demo script

1. Run `memory setup --developer --yes`
2. Run `memory wow`
3. Run `memory compile "fix billing export bug" --provider openai --budget 1500`
4. Run `memory doctor "fix billing export bug" --provider openai`
5. Open `.memory.cpp/reports/wow/wow-report.md`

## Benchmark script

```bash
cargo test --workspace
./scripts/release-candidate.ps1
memory bench
memory agents-score --for codex
```

## README screenshot checklist

- Terminal demo output
- Dashboard HTML report
- Context pack excerpt
- Token firewall report

## Launch drafts

Keep drafts local until explicitly approved:

- Hacker News: "Local repo memory + context compiler for AI coding agents"
- Reddit r/LocalLLaMA: local-first memory vault with MCP bridge
- Reddit r/rust: SQLite-backed CLI for AI context optimization
- Product Hunt: "Remember more. Send less. Run faster."

## Release checklist

- CI green on Linux, macOS, Windows
- Fresh-clone acceptance script passes
- Install scripts tested
- Checksums attached to release artifacts

## Positioning

See [competitive-positioning.md](./competitive-positioning.md).

## What memory.cpp is not

- Not a hosted SaaS memory service
- Not a vector database product
- Not a claim to compress closed-provider KV caches directly

## Why local-first matters

Developers should inspect, redact, export, and delete memory without sending repo data to a cloud by default.
