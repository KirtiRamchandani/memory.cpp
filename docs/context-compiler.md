# Context compiler and token firewall

memory.cpp can act as a local context compiler for AI coding tools. It does not replace a model provider, compress a provider KV cache directly, or upload your repo. It builds smaller, safer prompts from local repo memory so your AI assistant rereads less duplicated context.

Tagline:

```text
Remember more. Send less. Run faster.
```

## What this solves

Large AI coding prompts often contain repeated repo summaries, stale decisions, old tool logs, and accidental secrets. That wastes tokens and can confuse the model.

memory.cpp helps by keeping a local memory vault, then compiling only the useful pieces for the current task.

## Main commands

```bash
memory compile "fix checkout bug" --provider openai --budget 1500
memory prefill-report "fix checkout bug"
memory kv-budget "fix checkout bug" --max-kv-tokens 4096
memory signal-density "fix checkout bug"
memory batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
memory cache-plan "answer support ticket" --provider claude
memory cache-audit --file tests/fixtures/inference/provider_cache_bad_order.md --provider openai
memory kv-report "summarize customer history"
memory runtime-profile list
memory runtime-plan "fix checkout bug" --runtime llama.cpp
memory trace compress --file agent-log.txt
memory trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
memory mistake "Use pnpm only. Never npm."
memory doctor "add CSV export" --provider gemini
memory pack "fix checkout bug" --for codex --budget 1500
memory savings
```

## Memory vault

The vault is the existing local SQLite memory store. It uses current memory fields and metadata instead of a giant new schema.

It can hold:

- durable repo facts
- decisions
- bugs and fixes
- commands and test recipes
- mistakes that should not repeat
- compressed tool traces
- stale or superseded memories
- provenance and source links when available

It stays local by default under `.memory.cpp/`.

## Context compiler

`memory compile` takes a task, searches local memory, removes low-signal context, and prints a compact prompt.

Example:

```bash
memory compile "fix checkout bug" --provider openai --budget 1500
```

Output shape:

```text
# memory.cpp compiled context pack
Task: fix checkout bug
Provider: openai

## Critical facts
- ...

## Relevant decisions
- ...

## Prior failures/fixes
- ...

## Cache plan
OpenAI cache plan:
- Put stable repo memory, rules, decisions, and tool schemas first.
- Keep that stable prefix byte-for-byte stable between calls when possible.
- Put the latest user request, error, and tool output at the end.

TOKEN REPORT
Raw context available: 18320 tokens
Compiled context: 1240 tokens
Omitted: 17080 tokens
```

What just happened: memory.cpp selected useful memories, omitted stale or duplicate material, redacted secret-like content, and produced a prompt you can paste or save.

## Token firewall

`memory token-firewall` reports what was blocked before it reached the prompt.

```bash
memory token-firewall "fix checkout bug" --provider openai --budget 2000
```

It reports:

- duplicate context blocked
- stale context blocked
- tool-history bloat blocked
- secret-like strings blocked
- prompt-injection warnings
- estimated token reduction

This is a local prompt hygiene report. It does not send telemetry.

## Inference Cost Stack

`memory doctor "<task>" --provider <provider>` includes a unified `Inference Cost Stack` section:

```text
raw_context_tokens
compiled_context_tokens
fresh_suffix_tokens
cacheable_prefix_tokens
omitted_tokens
estimated_prefill_reduction_percent
estimated_kv_positions_avoided
signal_density_score
duplicate_context_tokens_blocked
stale_context_tokens_blocked
tool_trace_tokens_compressed
provider_cache_strategy
runtime_strategy
```

Use it when you want one compact view of why a task prompt is smaller, safer, and more cache-friendly than raw repo context.

## Prefill, KV budget, and signal density

```bash
memory prefill-report "fix checkout bug"
memory kv-budget "fix checkout bug" --max-kv-tokens 4096
memory signal-density "fix checkout bug"
```

These reports estimate prompt processing avoided, compile context under a KV-aware budget, and show useful signal versus duplicate, stale, low-relevance, tool-history, or secret-like prompt material.

## Cache router

`memory cache-plan` prints provider-specific layout advice.

```bash
memory cache-plan "answer support ticket" --provider claude
```

The plan separates stable prefix material from fresh task material. Provider wording is intentionally practical and conservative:

- OpenAI: stable prefix first, fresh request last.
- Claude: cache breakpoint style guidance.
- Gemini: cachedContent grouping guidance.
- Local/generic: prefix reuse, batching, and runtime hints.

`memory cache-audit` checks an existing prompt or context file for cache-breaking layout problems:

```bash
memory cache-audit --provider openai --file prompt.md
```

It detects dynamic text before stable prefixes, timestamps inside cacheable blocks, random IDs, changing tool outputs before stable memory, reordered blocks, and provider-specific mistakes.

## Batch planning

```bash
memory batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
```

Batch planning groups requests by shared stable prefix and reports per-request fresh suffix token counts, cache strategy, and estimated repeated tokens avoided.

## KV pressure report

`memory kv-report` estimates how many token positions were avoided by not sending unnecessary context.

```bash
memory kv-report "summarize customer history"
```

Important: this is an estimate. memory.cpp reduces KV pressure by preventing unnecessary tokens from entering the model. Runtime features such as KV quantization, prefix caching, batching, and speculative decoding remain separate runtime/provider features.

## Tool trace compressor

Long agent logs are often too noisy to paste into a model. Compress them first:

```bash
memory trace compress --file agent-log.txt
```

To queue a reviewable memory candidate from a trace:

```bash
memory trace learn --file agent-log.txt
```

To store an approved trace summary directly:

```bash
memory trace learn --file agent-log.txt --approve
```

## Mistake firewall

Teach the repo rules that should appear in future context packs:

```bash
memory mistake "Use pnpm only. Never npm."
memory mistakes
```

Mistake rules are tagged local memories. They are included when relevant and can be removed later:

```bash
memory mistakes remove <memory_id>
```

## Staleness and conflict hygiene

```bash
memory stale
memory conflicts
memory resolve <memory_id> --stale
memory clean stale --apply
```

These commands help prevent old decisions from polluting future context.

## Provider pack writers

```bash
memory pack "fix checkout bug" --for codex --budget 1500
memory pack "fix checkout bug" --for gemini --budget 1500
memory pack "fix checkout bug" --for cursor --output .memory.cpp/packs/cursor.md
```

Codex and Gemini defaults update guarded memory.cpp blocks in `AGENTS.md` and `GEMINI.md`. Other targets write Markdown packs. Use `--output` when you want an explicit file path.

## Savings report

```bash
memory savings
```

Savings are local estimates written to `.memory.cpp/savings.jsonl` when compile, firewall, KV, or AI doctor reports run.

## Runtime plan

```bash
memory runtime-profile list
memory runtime-plan "debug failing release" --runtime generic --budget 1500
memory runtime-plan "debug failing release" --runtime llama.cpp
memory runtime-plan "debug failing release" --runtime ollama
memory runtime-plan "debug failing release" --runtime vllm
memory runtime-plan "debug failing release" --runtime sglang
```

This prints runtime-neutral advice for context budgets, prefix reuse, KV quantization, speculative decoding, batching, and dynamic suffix placement. It also warns that memory.cpp does not implement low-level kernels by default.

## Trace rollup

```bash
memory trace-rollup --from agent-log.json --every 50
memory trace-rollup --stdin
```

Trace rollups compress older tool calls into decisions made, failed attempts, current state, remaining TODO, known bad paths, current error, and next action.

## Privacy notes

- No network calls are required.
- Secret-like content is redacted or omitted from compiled context.
- `.memoryignore` and `.gitignore` should be used for private paths.
- Generated packs should still be reviewed before sharing.

Next command:

```bash
memory doctor "your current task" --provider generic
```
