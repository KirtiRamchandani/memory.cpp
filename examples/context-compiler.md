# Context compiler example

Command:

```bash
memory compile "fix checkout bug" --provider openai --budget 1500
```

Output shape:

```text
# memory.cpp compiled context pack

Task: fix checkout bug
Provider: openai
Local-first note: generated locally. Review before sharing. Estimates are approximate.

## Critical facts
- CLI commands live in crates/memory-cli/src/main.rs (mem_cli_101, source: local memory)

## Relevant decisions
- Keep storage local-first in SQLite under .memory.cpp (mem_decision_022, source: docs/privacy.md)

## Prior failures/fixes
- Windows CI failed on CRLF newline style; .gitattributes now enforces LF (mem_bug_017, source: Git commit)

## Rules
- Mistake firewall rule: Use pnpm only. Never npm. (mem_rule_003, source: local memory)

## Cache plan
OpenAI cache plan:
- Put stable repo memory, rules, decisions, and tool schemas first.
- Keep that stable prefix byte-for-byte stable between calls when possible.
- Put the latest user request, error, and tool output at the end.

TOKEN REPORT
Raw context available: 18320 tokens
Compiled context: 1240 tokens
Cacheable prefix: 880 tokens
Fresh suffix: 260 tokens
Omitted: 17080 tokens
Estimated KV pressure avoided: 17080 token positions
Estimated context reduction: 93.2%
```

What just happened: memory.cpp built a compact prompt from local memory and explained what was omitted.
