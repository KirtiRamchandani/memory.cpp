# Offline Setup

## Goal

Use memory.cpp without cloud services.

## Commands

```bash
memory setup --offline --yes
memory embeddings set hash
memory privacy status
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

The repo uses local SQLite and lightweight local retrieval.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
