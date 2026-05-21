# Security Policy

`memory.cpp` is local-first repo memory. Security reports are welcome and should be handled carefully because the tool can store project context, commands, and generated assistant context packs.

## Supported versions

| Version | Supported |
| --- | --- |
| Current `main` | Security fixes accepted |
| Latest tagged release | Security fixes accepted |
| Older pre-1.0 tags | Best effort |

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability involving secret leakage, unsafe writes, path traversal, command execution, or integration config corruption.

Instead:

1. Use GitHub private vulnerability reporting if it is enabled for the repository.
2. If private reporting is unavailable, contact the maintainer privately before posting technical details publicly.
3. Include reproduction steps, platform, commit SHA, and whether `.memoryignore` or terminal memory was enabled.

## Response goals

For serious issues, the project should aim to:

- acknowledge the report within 7 days
- confirm impact and affected versions as quickly as practical
- ship a fix or mitigation before publishing exploit details
- document safe upgrade or purge steps

## Security boundaries

Expected safe defaults:

- local SQLite storage under `.memory.cpp/`
- no cloud upload by default
- terminal memory opt-in only
- MCP read-only by default
- risky writes approval-gated
- secrets redacted before previews/share artifacts where practical
- `.memoryignore` respected for memory capture where applicable

## Out of scope

These are not currently product surfaces:

- hosted SaaS
- enterprise sync
- team permissions
- billing
- plugin marketplace
- mobile packs
- AppSec scan packs

## Before sharing artifacts

Run:

```bash
memory privacy status
memory redact preview <path>
memory ignore list
```

Review generated files before posting them publicly.