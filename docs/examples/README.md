# Map Examples

These files are copied from a real `memory demo seed` run against the repository.

- `demo-evolution.mmd`: Mermaid evolution map
- `demo-decisions.md`: Markdown decision map
- `demo-architecture.mmd`: Mermaid architecture map

To regenerate them locally:

```bash
memory --db .memory.cpp/memory.db init --workspace demo
memory --db .memory.cpp/memory.db demo seed --workspace demo --path .
```
