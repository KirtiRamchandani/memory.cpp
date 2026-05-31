# Optimize AI context before asking an assistant

Goal: send less duplicated context while keeping the important repo memory.

## Commands

```bash
memory mistake "Use pnpm only. Never npm."
memory trace compress --file examples/agent-log.txt
memory compile "fix checkout bug" --provider openai --budget 1500
memory cache-plan "fix checkout bug" --provider claude
memory kv-report "fix checkout bug"
```

## Expected output

```text
TOKEN REPORT
Raw context available: 18320 tokens
Compiled context: 1240 tokens
Omitted: 17080 tokens
Estimated KV pressure avoided: 17080 token positions
```

## What happened

memory.cpp read local memory, filtered stale or duplicated context, included hard rules, and printed a compact task pack for your assistant.

## Privacy note

Everything is local by default. Review generated output before pasting it into any hosted AI tool.

## Next step

```bash
memory pack "fix checkout bug" --for codex --budget 1500
```
