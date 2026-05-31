# Token firewall example

Command:

```bash
memory token-firewall "fix checkout bug" --provider openai --budget 2000
```

Output shape:

```text
TOKEN FIREWALL REPORT
Task: fix checkout bug
Raw context available: 18320 tokens
Useful context selected: 1240 tokens
Duplicate context blocked: 3600 tokens
Stale context blocked: 900 tokens
Tool/history bloat blocked: 6200 tokens
Secret-like strings blocked: 0
Prompt-injection warnings: 0
Estimated reduction: 93.2%
```

What just happened: duplicated, stale, oversized, and unsafe context was kept out of the compiled prompt.
