# Launch checklist

Use this before a public release.

- README explains: Your repo remembers.
- Website builds and links pass.
- `install.sh --dry-run` works.
- `install.ps1 -DryRun` works.
- `cargo fmt --all -- --check` passes.
- `cargo check` passes.
- `cargo test` passes.
- `cargo build -p memory-cli` passes.
- Smoke scripts pass on PowerShell and Bash.
- Privacy docs explain purge, redaction, and local-first behavior.
- Known boundaries and intentionally omitted cloud/team features are documented.
- `SECURITY.md` is present and linked from README.
- `docs/api-stability.md` labels implemented, review-gated, opt-in, external, and internal surfaces.
- `docs/compatibility.md` says which platforms and integrations are supported.
- `docs/release-hardening.md` explains fresh clone and built-binary verification.
- Release workflow emits checksums.

## Virality and adoption loop

- [ ] `memory share status` generates a private-safe summary
- [ ] `memory pr summary` creates a PR-ready Markdown body
- [ ] `memory timeline week` works as a repo time machine
- [ ] `memory handoff new-dev` creates a sanitized local bundle
- [ ] `memory adoption checklist` gives the next activation step
- [ ] `memory release-check` passes before tagging
- [ ] `./scripts/release-candidate.sh` or `./scripts/release-candidate.ps1` passes
- [ ] README hot topics, quick start, docs links, limitations, and product surface labels are current
