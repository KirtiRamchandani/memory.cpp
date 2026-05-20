# Use memory.cpp with Cursor

## Goal

Give Cursor a local project memory context.

## Commands

```bash
memory setup --ai-coding
memory attach cursor
memory dev context --for cursor
memory privacy status
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

Cursor gets read-only local memory access and a clean context pack.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
