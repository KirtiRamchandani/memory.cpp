# Providers

memory.cpp is provider-aware without becoming a provider SDK.

## Supported pack targets

- Codex: `memory pack "<task>" --for codex`
- Claude: `memory pack "<task>" --for claude`
- Gemini: `memory pack "<task>" --for gemini`
- Cursor: `memory pack "<task>" --for cursor`
- Continue: `memory pack "<task>" --for continue`
- MCP: `memory pack "<task>" --for mcp`

Attach dry-runs are available for the same developer surfaces:

```bash
memory attach gemini --dry-run
memory attach mcp --dry-run
memory attach all --dry-run
```

`attach all` expands to Cursor, Claude, Gemini, VS Code, Codex, Continue, MCP, and Ollama. It remains local-first and read-only by default.
- Generic/local: `memory pack "<task>" --for generic`

## Cache planning

Use `memory cache-plan`, `memory cache-audit`, `memory cache-hash`, and `memory cache-stability` to keep stable context separate from fresh suffixes.

memory.cpp does not claim provider-side cache hits. It prepares cache-friendly prompts and reports risks locally.
