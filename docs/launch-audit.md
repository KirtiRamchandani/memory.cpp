# Launch audit

This page maps the public launch objective to current repo evidence. It is intentionally practical: every row points to code, docs, smoke coverage, CI, or GitHub state that can be inspected again.

Current public evidence:

- Repository: https://github.com/KirtiRamchandani/memory.cpp
- Visibility: public
- Latest verified commit before this audit pass: `26ca1c9440e5688eb7b65407df16a6d7795639c4`
- CI run: https://github.com/KirtiRamchandani/memory.cpp/actions/runs/26766324909
- Pages run: https://github.com/KirtiRamchandani/memory.cpp/actions/runs/26766324957

## Product promise

memory.cpp is a local-first AI memory and context control plane. It helps developers remember more, send less, and run faster by compiling local repo memory into smaller provider-ready context packs.

## Evidence matrix

| Objective area | Current evidence |
| --- | --- |
| Public repository | GitHub reports `visibility: PUBLIC` for `KirtiRamchandani/memory.cpp`. |
| No cloud by default | README, privacy docs, setup output, smoke output, and CLI reports say local-only/no upload by default. |
| No telemetry or accounts | README and docs describe local SQLite storage and no account requirement. |
| No hardware-vendor claims | README/docs use provider-neutral and runtime-neutral wording. |
| No direct closed-provider KV compression claim | README/docs/CLI say memory.cpp reduces KV pressure by reducing unnecessary tokens before inference. |
| No staged-release launch wording | README/docs/website avoid tentative launch framing; `memory embeddings explain` uses opt-in/local wording. |
| One-command wow loop | `memory wow`, `memory autopilot`, and `memory ship-demo` are implemented in `crates/memory-cli/src/main.rs` and covered by `scripts/smoke.ps1` and `scripts/smoke.sh`. |
| Public terminal demo artifacts | `scripts/demo-terminal.ps1` and `scripts/demo-terminal.sh` write deterministic terminal-demo artifacts under `.memory.cpp/reports/demo/` and detect optional recording tools without installing them. |
| Fresh-clone built-binary proof | `scripts/fresh-clone-acceptance.ps1` and `scripts/fresh-clone-acceptance.sh` clone the committed repo, build `memory-cli`, and run the release acceptance loop from the built `memory` binary. Dry-run mode is smoke-covered. |
| Universal memory vault | `remember`, `recall`, `forget`, `update-memory`, `memories`, and `profile` commands are implemented and smoke-covered. |
| Context compiler | `compile`, `pack`, `explain-compile`, `token-firewall`, `kv-report`, `kv-budget`, `prefill-report`, and `signal-density` are implemented and smoke-covered. |
| Provider packs | `pack --for generic/gemini/mcp` is smoke-covered; docs describe Codex, Claude, Gemini, Cursor, Continue, MCP, and generic packs. |
| Attach flows | `attach cursor --dry-run`, `attach all`, `attach gemini --dry-run`, `attach mcp --dry-run`, `attach status`, `attach verify`, and detach dry-run are smoke-covered. |
| Token firewall and inference reports | `doctor --json`, `token-firewall`, `prefill-report`, `kv-budget`, `signal-density`, `batch-plan`, and `runtime-plan` are smoke-covered. |
| Cache router | `cache-plan`, `cache-audit`, `cache-hash`, and `cache-stability` are smoke-covered with inference fixtures. |
| Trust and safety | `privacy`, `safety`, `trust-report`, `redactions`, `mcp-scan`, `mcp-harden`, `sign`, `verify`, `quarantine`, and `review` are smoke-covered. |
| Mistake firewall | `mistake`, `mistakes`, and relevant pack inclusion are implemented; smoke records and lists mistake rules. |
| Flight recorder | `flight start`, `flight summarize`, and `flight stop` are smoke-covered. |
| Debuggable memory | `context-diff latest previous`, pack-to-pack context diff, `blame --pack`, `explain-pack`, `test`, and `ci-check` are smoke-covered. |
| Ask/proactive memory | `ask`, `suggest`, `warnings`, `next`, and `proactive` are implemented; `ask` and suggestion workflows are smoke-covered. |
| Ingestion and docs memory | `ingest file`, `docs list`, `docs summarize`, and `docs search` are smoke-covered. |
| Shared context | `shared-context status` and `shared-context export` are smoke-covered. |
| Static visual reports | `heatmap --html`, `report --html`, `dashboard --html`, `map`, `show-map`, and map exports are smoke-covered. |
| Agent readiness and badges | `agents-score` and `badge` are implemented and smoke-covered. |
| Recipes | `recipe list` and `recipe apply coding-agent` are smoke-covered; docs include community recipes. |
| PR/Git automation | `pr summary`, `pr-comment`, `pr-context`, `git-learn`, and `branch-summary` are smoke-covered. |
| Examples and benchmarks | `demo`, `demo multi-model`, `examples list/run`, `bench-context`, and `bench --json` are smoke-covered. |
| Public API | `docs/api.md` and `crates/memory-core/src/api.rs` expose compile, pack, doctor, estimate, audit, trace, batch, ask, test, and readiness APIs. |
| Final validation | CI runs format, Clippy with warnings as errors, tests, and OS-specific smoke on Linux, macOS, and Windows. |

## Exact final acceptance commands

The smoke scripts now include the exact acceptance shapes:

```bash
memory init
memory demo
memory demo multi-model
memory doctor "fix the billing export bug" --provider openai --json
memory pack "fix the billing export bug" --for codex --budget 1500
memory attach all
memory preflight --for codex "fix checkout bug"
memory agents-score
memory cache-audit --provider openai
memory context-diff latest previous
memory ask "what broke last time billing changed?"
memory test
memory bench --json
```

The source-run form is used in smoke so the command path is tested before packaging:

```bash
cargo run -p memory-cli -- --db "$DB" ...
```

## Evidence still worth strengthening

These are not blockers for the current local-first launch, but they are the next best proof upgrades:

- Run the fresh-clone acceptance script against the public GitHub URL after every tagged release.
- Add a rendered screenshot or GIF after the terminal-demo transcript is approved.
- Add package-manager release notes for Homebrew or prebuilt binary distribution once release assets are published.
- Add stronger fixture assertions around exact provider pack marker blocks for Codex and Gemini.
