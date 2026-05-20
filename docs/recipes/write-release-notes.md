# Write Release Notes

## Goal

Generate a changelog from repo memory.

## Commands

```bash
memory dev changelog --since v0.2.1
memory git release-notes --since v0.2.1
memory dev release-notes
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

The tool groups added, changed, fixed, docs, internal, and breaking notes.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
