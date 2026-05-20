# Understand a New Repo

## Goal

Get an instant repo briefing.

## Commands

```bash
memory setup --developer
memory dev explain-repo .
memory dev onboard --output markdown
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

memory.cpp inspects repo shape, important files, commands, storage, risks, and roadmap.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
