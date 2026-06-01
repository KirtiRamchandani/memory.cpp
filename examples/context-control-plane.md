# Context control plane example

```bash
memory memories list
memory explain-compile "fix checkout bug" --provider openai
memory trust-report
memory flight start --goal "fix checkout bug" --tool codex
memory agents-score --for codex
memory demo multi-model
memory docs search "context compiler"
memory examples run billing-export
```

Output shape:

```text
memory.cpp leaderboard
- duplicate context
- stale memory
- tool/result/history bloat

AI Agent Ready score
score: 78%
```

Everything stays local unless you explicitly share an artifact.
