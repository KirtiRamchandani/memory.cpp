# Repo time machine

`memory timeline`, `memory rewind`, and `memory changed` show what happened, why it changed, and what to do next.

```bash
memory timeline week
memory timeline since 2026-05-01 --output .memory.cpp/share/timeline.md
memory rewind last-week
memory changed --since "7 days ago"
```

The timeline combines local memory events with Git commits when available and degrades gracefully in tiny repos.
