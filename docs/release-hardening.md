# Release hardening

This checklist keeps `memory.cpp` boring to install, test, and ship.

## Required local gate

Run from a clean checkout before tagging a release:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p memory-cli
git diff --check
```

Or use:

```bash
./scripts/release-candidate.sh
```

PowerShell:

```powershell
./scripts/release-candidate.ps1
```

## Fresh clone verification

Use a temporary folder, not your working checkout:

```bash
git clone https://github.com/KirtiRamchandani/memory.cpp.git memory.cpp-rc
cd memory.cpp-rc
cargo build -p memory-cli
cargo test --workspace
./scripts/smoke.sh
```

On Windows:

```powershell
git clone https://github.com/KirtiRamchandani/memory.cpp.git memory.cpp-rc
Set-Location memory.cpp-rc
cargo build -p memory-cli
cargo test --workspace
./scripts/smoke.ps1
```

## Built-binary verification

Always test the release binary, not only `cargo run`:

```bash
cargo build --release -p memory-cli
./target/release/memory --help
./target/release/memory setup --developer --yes
./target/release/memory doctor
```

Windows:

```powershell
cargo build --release -p memory-cli
./target/release/memory.exe --help
./target/release/memory.exe setup --developer --yes
./target/release/memory.exe doctor
```

## Package verification

This repo currently publishes GitHub release archives, not npm packages.

Before attaching release assets, confirm:

- release archive contains exactly one `memory` binary or `memory.exe`
- checksum file exists for each archive
- README, LICENSE, SECURITY.md, install docs, and changelog are present in the repo
- no absolute local paths appear in docs, examples, scripts, or generated release notes
- examples are concise and safe to publish
- source maps are not applicable to the Rust CLI
- C API headers are present under `include/`

## Cross-platform verification

GitHub CI should pass on:

- Linux
- macOS
- Windows

Local smoke coverage should include:

- setup
- privacy status
- map generation
- context generation
- terminal record/search
- Git watch dry-run
- CI ingest/explain
- release-check

## Release blocking failures

Do not tag a release if any of these are true:

- `cargo fmt --all -- --check` fails
- `cargo clippy --workspace --all-targets -- -D warnings` fails
- `cargo test --workspace` fails
- smoke scripts fail in a supported environment
- docs or website link checks fail
- generated archives lack checksums
- SECURITY.md is missing
- README quickstart does not work from a fresh clone