# Recover a Fix

## Goal

Find how an error was fixed before.

## Commands

```bash
memory dev recall-error "ECONNRESET"
memory terminal last-error
memory dev next
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

The CLI searches previous errors, fixes, commands, and related memories.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
