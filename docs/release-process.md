# Release process

This is the release process for the Rust `memory.cpp` CLI and C API workspace.

## 1. Prepare

```bash
git status --short --untracked-files=all
./scripts/release-candidate.sh
```

PowerShell:

```powershell
./scripts/release-candidate.ps1
```

## 2. Review docs

Confirm these are current:

- README.md
- SECURITY.md
- docs/changelog.md
- docs/roadmap.md
- docs/compatibility.md
- docs/api-stability.md
- docs/limitations.md
- docs/release-hardening.md

## 3. Build release artifacts

The release workflow builds platform archives and checksum files when a `v*` tag is pushed.

Manual local check:

```bash
cargo build --release -p memory-cli
```

## 4. Tag

```bash
git tag v0.x.y
git push origin v0.x.y
```

## 5. Verify GitHub release

Confirm:

- Linux, macOS, and Windows artifacts exist
- checksum files exist
- combined `checksums.txt` exists
- generated release notes are accurate
- opt-in/review-gated/external surfaces are described accurately

## 6. After release

- Run install script dry-runs.
- Download one release artifact and run `memory doctor`.
- Update launch notes if this is a public release.
- Open issues for any deferred work rather than hiding it in release notes.
