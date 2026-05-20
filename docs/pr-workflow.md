# PR workflow

`memory pr` generates local Markdown for pull request bodies and review comments.

```bash
memory pr summary --base main
memory pr checklist --output .memory.cpp/share/pr-checklist.md
memory pr comment --output .memory.cpp/share/pr-comment.md
```

## Output shape

- What changed
- Why it matters
- Related memories
- Risky files
- Tests to run
- Docs to update

No network is required; it uses the Git CLI and local memory.
