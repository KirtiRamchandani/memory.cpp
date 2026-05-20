# Privacy

memory.cpp is local-first.

## Where data lives

```bash
memory where
```

Default paths:

- .memory.cpp/memory.db
- .memory.cpp/memory-config.json
- .memory.cpp/audit/
- .memory.cpp/runtime/
- .memory.cpp/terminal/

## Delete everything

```bash
memory privacy purge --yes
```

Or remove .memory.cpp/ manually.

## What is never stored by default?

- Terminal history unless enabled.
- MCP writes unless explicitly allowed.
- Cloud copies.
- Files ignored by .memoryignore during import/watch.
