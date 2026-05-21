# Dogfooding guide

Use `memory.cpp` on this repository before each public release.

## Five-minute dogfood loop

```bash
memory setup --developer --yes
memory dev morning
memory context write --for codex --output .memory.cpp/context/codex.md
memory map --type evolution --output html --save .memory.cpp/maps/evolution.html
memory share status --output .memory.cpp/share/project-memory-summary.md
```

## PR dogfood loop

```bash
memory pr summary --base main --output .memory.cpp/share/pr-summary.md
memory pr checklist --base main
memory dev health
memory release-check
```

## Case study template

Use this structure when writing a real case study:

```markdown
# Case study: memory.cpp on <repo>

## Before

- What was hard to remember?
- Which commands or decisions were scattered?
- Where did AI assistants lack context?

## After

- Which memories were captured?
- Which context pack helped?
- Which project map explained the repo?
- What was the exact next command?

## Result

Before memory.cpp: <manual archaeology>
After memory.cpp: <one command or artifact>
```

## What to watch

- Did output stay short by default?
- Did every command suggest a practical next step?
- Did redaction avoid secrets?
- Did maps cite sources?
- Did context packs avoid unsupported claims?