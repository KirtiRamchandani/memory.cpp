# Safety

`memory.cpp` is only useful if people trust it.

## What v0.2.1 does today

- local-first storage
- SQLite-backed single-file database
- sensitive-data detection on candidate memory capture
- review inbox for uncertain or sensitive memory
- MCP read-only default
- workspace-scoped MCP access
- audit log for agent access
- response redaction for MCP output
- `.memoryignore` and `.gitignore` support for import/watch discovery

## Review inbox

Low-confidence or sensitive candidate memories are queued instead of stored directly.

Use:

```bash
memory --db .memory.cpp/memory.db inbox list --workspace demo
memory --db .memory.cpp/memory.db inbox approve <id>
memory --db .memory.cpp/memory.db inbox reject <id>
```

## `.memoryignore`

You can keep secrets and irrelevant files out of memory ingestion with a repo-local `.memoryignore` file.

Example:

```text
.env
*.pem
*.key
secrets/
node_modules/
target/
```

Both `memory import` and `memory dev watch` respect `.memoryignore` and `.gitignore`.

Useful commands:

```bash
memory ignore init
memory ignore check README.md
memory --db .memory.cpp/memory.db import . --workspace demo --preview-redactions
```

## What is intentionally not claimed yet

These are still important future layers:

- encrypted live database storage
- team approval workflows
- sync
- organization policy inheritance
- advanced secret classifiers

That is why the project should still be presented as a strong v0.2 foundation rather than a finished universe.
