# Private Local Setup

## Goal

Use conservative local defaults.

## Commands

```bash
memory setup --private --offline
memory privacy status
memory ignore init
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

The repo gets local storage, ignore rules, and privacy-first defaults.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
