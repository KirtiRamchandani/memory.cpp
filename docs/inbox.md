# Candidate inbox

The inbox is how memory.cpp stays useful without being creepy.

## Commands

```bash
memory inbox
memory inbox stats
memory inbox explain <id>
memory inbox edit <id>
memory inbox approve <id>
memory inbox reject <id>
memory inbox approve-all --confidence-above 0.9
```

## What just happened?

Automatic extraction creates candidates. You decide what becomes durable memory.

## Common mistake

Do not approve secrets. If a candidate looks sensitive, reject or edit it first.

## Public review flow

```bash
memory inbox review
memory inbox rules
memory inbox rules add "docs/**" --action review
memory inbox approve-all --confidence-above 0.9 --dry-run
```

What just happened: review shows one useful candidate at a time, rules document how automatic memory should behave, and approve-all can be previewed before it stores anything.

Safe default: secrets and ignored paths should never be stored; sensitive candidates should stay in review.
