# Compatibility

This page states what `memory.cpp` is expected to run on today.

| Runtime or tool | Status | Notes |
| --- | --- | --- |
| Rust stable | Supported | CI uses the stable toolchain with rustfmt and clippy. |
| Linux x86_64 | Supported | CI and release workflow build/test on Ubuntu. |
| macOS x86_64/arm64 users | Supported by source build | CI uses GitHub-hosted macOS; release artifact naming is currently x86_64. |
| Windows x86_64 | Supported | CI and PowerShell smoke run on Windows. |
| SQLite | Supported | Bundled through `rusqlite` with the bundled feature. |
| Git CLI | Optional | Git commands degrade gracefully outside a Git repo. |
| Ollama | Optional | Only used when configured or checked by doctor/setup. |
| Cursor | Review-gated integration | Attach flow is dry-run first and read-only by default. |
| Claude Desktop | Review-gated integration | Attach flow is dry-run first and read-only by default. |
| VS Code / Continue | Review-gated integration | Snippet/config generation where safe. |
| Codex | Context workflow | Direct attach may be a context file rather than config mutation. |
| MCP | Review-gated | Read-only by default. Write tools require explicit approval. |
| Terminal memory | Opt-in | Shell integration and command recording require enablement. |
| CI memory | Supported | Generic and GitHub Actions log parsing where simple. |
| Browser dashboard | Supported local static UI | Static/local UI only. |
| Hosted SaaS | Not supported | Intentionally out of scope. |
| Team sync | Not supported | Use handoff/export flows for now. |
| Mobile packs | Not supported | Intentionally deferred. |
| Fuzzing/AppSec packs | Not supported | Intentionally deferred. |

## Low-RAM mode

The default hash embedding path is intentionally small. Avoid optional providers unless you need them.

Recommended low-RAM setup:

```bash
memory setup --offline --yes
memory embeddings set hash
memory dev morning
```

## Line endings

The repository enforces LF for Rust, TOML, Markdown, shell scripts, YAML, HTML, CSS, and JS through `.gitattributes`. This prevents Windows rustfmt failures from CRLF line endings.
