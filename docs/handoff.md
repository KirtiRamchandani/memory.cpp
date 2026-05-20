# Local handoff bundles

`memory handoff` creates a sanitized local bundle for a new developer, reviewer, maintainer, or AI agent without adding team sync.

```bash
memory handoff new-dev --output .memory.cpp/handoff
memory handoff reviewer --private-safe
memory handoff import --output .memory.cpp/handoff
```

The bundle includes project summary, commands, important files, recent memory, and a redaction report note. Review before sharing.
