# API stability

`memory.cpp` keeps the public surface predictable: existing command names remain available, JSON output grows additively where practical, and local data stays readable across releases.

## Surface labels

| Label | Meaning |
| --- | --- |
| Implemented | Available in the CLI/API and covered by docs, tests, or smoke scripts. |
| Review-gated | Available, but edits or config writes require dry-run, review, or explicit approval. |
| Opt-in | Off by default until the user enables it. |
| External | Depends on a user-installed tool or runtime. |
| Internal | Not intended for users. No compatibility promise. |

## Public surface map

| Surface | Status | Notes |
| --- | --- | --- |
| `memory remember` | Implemented | Core write path. |
| `memory search` | Implemented | Core recall path. |
| `memory explain` | Implemented | Beginner explanation surface. |
| `memory edit/restore/history` | Implemented | Versioned memory management. |
| SQLite storage schema | Implemented | Core tables stay small; metadata may expand additively. |
| Rust API and C API | Implemented | Useful for embeddings and host integrations. |
| `memory wow/autopilot/ship-demo` | Implemented | One-command launch/demo loop and local artifacts. |
| `memory dev morning/resume/next` | Implemented | Daily developer workflow output. |
| `memory context` and `memory dev context` | Implemented | Context sections and budgets are explicit. |
| `memory map` | Implemented | Static HTML, Markdown, Mermaid, and JSON output. |
| `memory inbox` | Review-gated | Candidate review model is approval-first. |
| `memory git watch` and `memory watch` | Review-gated | Automatic capture writes candidates first. |
| `memory terminal` | Opt-in | Command capture stays disabled until enabled. |
| `memory ci` | Implemented | Lightweight log memory, not a CI platform. |
| Embedding provider registry | External | Hash provider is default; ONNX Runtime is not bundled. |
| Dashboard/static website | Implemented | Static and local-first; not a hosted app. |

## Compatibility promises

- Existing command names should not be removed without a deprecation note.
- JSON output should be extended rather than reshaped when practical.
- Local data should remain readable across minor releases or include migration guidance.
- Risky writes stay approval-gated by default.
- New automatic capture should create candidates before approved memories unless the user explicitly changes policy.

## Internal surfaces

These are internal unless documented elsewhere:

- private helper functions in `crates/memory-cli/src/main.rs`
- unexported Rust modules
- generated demo artifacts
- `.memory.cpp/runtime/` files
- temporary map/context output names

## Deprecation policy

When a public command or flag changes:

1. Keep the old spelling for at least one minor release when practical.
2. Print a clear replacement command.
3. Update README, CLI docs, examples, and smoke tests.
4. Mention the change in `docs/changelog.md`.
