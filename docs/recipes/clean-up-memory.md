# Clean Up Memory

## Goal

Review and safely clean local state.

## Commands

```bash
memory inbox stats
memory inbox clear-rejected
memory clean
memory privacy status
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

Only safe temporary/runtime data and rejected candidates are cleaned unless you explicitly purge.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
