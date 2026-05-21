# Contributing

Thanks for helping build `memory.cpp`.

The project lane is intentionally narrow:

> memory.cpp helps your repo remember what happened, why it changed, and what to do next - locally, safely, and simply.

## Local setup

Install Rust from `https://rustup.rs/`, then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p memory-cli
```

On Windows:

```powershell
./scripts/verify.ps1
```

For release-candidate validation:

```bash
./scripts/release-candidate.sh
```

PowerShell:

```powershell
./scripts/release-candidate.ps1
```

## Project taste

Prefer:

- local-first behavior
- low dependency count
- explicit APIs and exact commands
- beginner-friendly output
- small durable schemas
- redaction and approval gates
- docs/examples with every user-facing change

Avoid:

- hosted cloud requirements
- broad plugin frameworks
- enterprise/team sync in the core path
- mobile/AppSec/fuzzing packs before the developer core is loved
- heavyweight ML dependencies by default
- auto-saving secrets or risky memories

## Pull requests

Good pull requests include:

- a focused change
- tests for behavior changes
- docs updates for public commands or flags
- examples when the change is user-facing
- benchmark notes for performance-sensitive paths
- clear maturity labels for beta or experimental surfaces

## Adding a command

Before adding a command, ask:

- Does this help developers resume work, understand repos, remember fixes, explain decisions, or give AI assistants accurate context?
- Can an existing command be polished instead?
- Does it degrade gracefully when the repo has little data?
- Does it print one exact next command?
- Does it avoid network or cloud requirements by default?

## Documentation expectations

If behavior changes, update the relevant docs under `docs/`, examples under `examples/`, website links under `website/`, and smoke coverage when practical.

## Security

Do not include secrets, tokens, private keys, personal absolute paths, or real private repo data in tests, examples, docs, screenshots, or launch assets. See `SECURITY.md`.