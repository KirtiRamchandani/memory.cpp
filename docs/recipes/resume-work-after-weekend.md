# Resume Work After a Weekend

## Goal

Rebuild the thread after being away.

## Commands

```bash
memory dev morning
memory dev resume
memory dev next
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

memory.cpp recalls recent work, branch state, candidates, and next steps.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
