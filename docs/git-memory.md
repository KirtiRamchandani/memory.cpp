# Git memory

Git tells you what changed. memory.cpp helps remember why it mattered.

## Try this now

```bash
memory git summary --since 7d
memory git ingest --since 7d --workspace default
memory git watch --once
```

## What happens?

- summary reads recent commits.
- ingest turns useful commit messages into memory candidates.
- watch --once records the current branch/head as a baseline.

## Useful commands

```bash
memory git decisions --since 30d
memory git bugs --since 30d
memory git map --output html --save .memory.cpp/demo/git-map.html
```

Safe default: git watch writes local state under .memory.cpp/git-watch/.
