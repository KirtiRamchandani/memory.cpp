# API stability

`memory.cpp` is pre-1.0, but the public surface should still be predictable.

## Stability labels

| Label | Meaning |
| --- | --- |
| Stable | Expected to remain compatible across minor releases. Breaking changes require clear migration notes. |
| Beta | Useful and intended to stay, but output details or flags can change before 1.0. |
| Experimental | Available for feedback. Names, output, behavior, or storage details may change. |
| Internal | Not intended for users. No compatibility promise. |

## Public surface map

| Surface | Stability | Notes |
| --- | --- | --- |
| `memory remember` | Stable | Core write path. |
| `memory search` | Stable | Core recall path. |
| `memory explain` | Stable | Beginner explanation surface. |
| `memory edit/restore/history` | Stable | Versioned memory management. |
| SQLite storage schema | Stable core, evolving metadata | Core tables stay small; metadata may expand. |
| C API in `include/` | Beta | Useful for embeddings and host integrations. |
| `memory dev morning/resume/next` | Beta | Daily developer workflow output may improve over time. |
| `memory context` and `memory dev context` | Beta | Context sections and budgets may evolve. |
| `memory map` | Beta | Formats are useful but visual output can improve. |
| `memory inbox` | Beta | Candidate review model is expected to remain. |
| `memory git watch` and `memory watch` | Beta | Daemon behavior is intentionally conservative. |
| `memory terminal` | Experimental | Opt-in command capture. |
| `memory ci` | Experimental | Lightweight log memory, not a CI platform. |
| Embedding provider registry | Experimental | Hash provider is default; ONNX Runtime is not bundled. |
| Dashboard/static website | Experimental | Static and local-first; not a hosted app. |

## Compatibility promises before 1.0

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