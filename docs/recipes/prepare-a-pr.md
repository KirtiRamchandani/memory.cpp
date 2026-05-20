# Prepare a PR

## Goal

Create a practical PR summary.

## Commands

```bash
memory dev pr-summary
memory dev review
memory dev context --for codex
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

memory.cpp summarizes changed intent, risks, tests, docs, and relevant decisions.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
