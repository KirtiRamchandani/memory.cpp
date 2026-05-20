# Generate a Project Map

## Goal

Create a shareable HTML evolution map.

## Commands

```bash
memory demo seed --workspace demo --path .
memory map --type evolution --output html --save .memory.cpp/demo/evolution.html
memory open --print map
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

A self-contained HTML map is generated from local memories and optional Git signals.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
