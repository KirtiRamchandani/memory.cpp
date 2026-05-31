# Examples

`memory.cpp` examples are intentionally short. Each one shows the command, the shape of the output, and what to try next.

## Daily recap

```bash
memory dev morning
```

Output shape:

```text
What you were doing
- Parser cleanup and developer-ready docs.

What changed recently
- Branch: main
- Uncommitted files: 3

What to do next
- Run: memory dev next
```

## Why did this exist?

```bash
memory map why "SQLite storage"
```

Output shape:

```text
Why SQLite storage exists
- Local-first database that travels with the repo.
- Supports provenance, workspaces, candidate review, and maps.

Try next
- memory map impact "SQLite storage"
```

## What should Codex know?

```bash
memory dev context --for codex
```

Output shape:

```text
Project context for Codex
- Summary
- Recent decisions
- Important files
- Commands to run
- Known pitfalls
- Privacy note
```

## Compile context before asking AI

```bash
memory compile "fix checkout bug" --provider openai --budget 1500
memory token-firewall "fix checkout bug"
memory kv-report "fix checkout bug"
memory prefill-report "fix checkout bug"
memory kv-budget "fix checkout bug" --max-kv-tokens 4096
memory signal-density "fix checkout bug"
```

Output shape:

```text
TOKEN REPORT
Raw context available: 18320 tokens
Compiled context: 1240 tokens
Omitted: 17080 tokens
Estimated KV pressure avoided: 17080 token positions
```

## Plan cache-friendly batches

```bash
memory batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
memory cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
memory trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
```

Output shape:

```text
BATCH PLAN
- group checkout-fixes
- shared stable prefix token count: 240
- estimated repeated tokens avoided: 480

CACHE AUDIT
Cache hit risk: high
Problems:
- dynamic text appears before stable/cacheable prefix
```

## What broke before?

```bash
memory dev recall-error "ECONNRESET"
```

Output shape:

```text
Previous fixes
- Restart the local test database.
- Wait for the service port to be free.
- Re-run the focused test command.
```

## More examples

The `examples/` directory contains concise static examples for:

- `memory dev morning`
- `memory dev next`
- `memory dev context --for cursor`
- `memory compile`
- `memory token-firewall`
- `memory kv-report`
- `memory prefill-report`
- `memory kv-budget`
- `memory signal-density`
- `memory batch-plan`
- `memory cache-audit`
- `memory trace-rollup`
- `memory trace compress`
- `memory mistake`
- `memory privacy status`
- `memory ci explain-failure`
- `memory git summary`
- HTML and Markdown project maps
