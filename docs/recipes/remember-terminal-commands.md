# Remember Terminal Commands

## Goal

Opt in to command recall.

## Commands

```bash
memory terminal enable
memory terminal record --command "cargo test" --exit-code 0
memory terminal search "how did I run tests?"
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

Successful and failed commands are stored locally with redaction.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
