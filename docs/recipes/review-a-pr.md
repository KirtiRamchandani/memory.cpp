# Review a PR

## Goal

Recall style and risk notes before review.

## Commands

```bash
memory dev review
memory dev pr-summary
memory dev context --for generic
```

## Expected Output Shape

```text
workspace: current
summary: short practical notes
next: exact command to run
```

## What Happened

Review memory surfaces previous comments, common mistakes, owner notes, and practical risks.

## Privacy Note

The workflow uses the local `.memory.cpp/` store. Use `memory privacy status` to see paths and `memory privacy purge --yes` to delete local data.
