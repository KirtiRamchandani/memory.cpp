# Demo script

Run this local demo without API keys:

```bash
memory init
memory demo seed --workspace demo
memory doctor "fix the billing export bug" --provider openai
memory pack "fix the billing export bug" --for codex --budget 1500
memory preflight --for codex "fix the billing export bug"
memory agents-score --for codex
memory bench
```

Expected result: token waste blocked, KV pressure estimated, provider cache plan generated, stale memory excluded, hard rules included, and static reports available under `.memory.cpp/`.
