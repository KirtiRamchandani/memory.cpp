# Security notes

This document explains the practical threat model for `memory.cpp`.

## Default trust model

- Data stays local by default.
- `.memory.cpp/` is the default project data directory.
- MCP is read-only by default.
- Terminal memory is opt-in.
- Candidate memories can be reviewed before approval.
- Redaction is applied before previews and shareable artifacts where practical.

## Data memory.cpp may store

- Project decisions and rationale.
- Bug/fix notes.
- Commands you explicitly record or enable terminal memory for.
- Git-derived summaries and candidate memories.
- CI logs you ingest.
- Context packs, maps, receipts, and generated artifacts.

## Data it should not store by default

- API keys.
- Bearer tokens.
- Cookies.
- Passwords.
- Private keys.
- `.env` secrets.
- Ignored paths from `.memoryignore` and `.gitignore` where applicable.

## Recommended checks before sharing artifacts

```bash
memory privacy status
memory redact preview README.md
memory ignore list
memory share status --private-safe --output .memory.cpp/share/project-memory-summary.md
```

## Integration safety

MCP and editor integrations should expose read-only tools by default:

- memory search
- memory context
- memory map
- memory timeline
- memory explain

Write tools such as add, patch, or forget should require explicit user approval or remain disabled.

## Vulnerability reporting

Use the process in [../SECURITY.md](../SECURITY.md).