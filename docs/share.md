# Shareable artifacts

`memory share` creates private-safe Markdown or HTML files that can go into READMEs, PRs, issues, or team chats.

## Commands

```bash
memory share status
memory share map --output .memory.cpp/share/project-evolution-map.html
memory share context --output .memory.cpp/share/ai-context-pack.md
memory share onboarding --private-safe
memory share pr --no-brand
memory share release
```

## What happens

memory.cpp reads local memories, Git state, terminal/CI signals when available, and redacts secret-looking text before writing artifacts.

## Safe default

Artifacts are local files. Review them before sharing.
