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
- Known beta limitations are documented.
- Release workflow emits checksums.