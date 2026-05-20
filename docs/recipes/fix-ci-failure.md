# Fix a CI Failure

## Goal

Import a CI log and recall similar failures.

## Commands

```bash
memory ci ingest ./ci.log
memory ci explain-failure
memory ci fix-history
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

Failure lines become local memories with source links and suggested next steps.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
