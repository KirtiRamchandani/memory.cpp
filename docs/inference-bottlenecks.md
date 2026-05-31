# Inference bottlenecks

memory.cpp is a local-first context control plane. It does not claim exact speedups and it does not directly compress a closed-provider KV cache. It reduces prompt-side inference cost by compiling smaller, higher-signal context before the model sees it.

## The eight bottlenecks

1. Prefill cost: `memory prefill-report` estimates prompt processing avoided before generation.
2. KV cache memory: `memory kv-report` and `memory kv-budget` estimate token positions avoided by sending less context.
3. Attention over long context: `memory signal-density` shows signal versus duplicated, stale, or low-relevance prompt material.
4. Tool/result/history bloat: `memory trace compress` and `memory trace-rollup` turn noisy agent sessions into compact state.
5. Batching effects: `memory batch-plan` groups requests by shared stable prefix and fresh suffixes.
6. Speculative decoding: `memory runtime-plan` provides vendor-neutral serving hints for shorter prompts.
7. Cache hits/misses: `memory cache-plan` and `memory cache-audit` detect unstable cache prefixes.
8. Model architecture/serving engine: `memory runtime-profile list` and runtime-specific plans document what memory.cpp can prepare versus what the runtime must implement.

## Commands

```bash
memory doctor "fix checkout bug" --provider openai
memory prefill-report "fix checkout bug"
memory kv-budget "fix checkout bug" --max-kv-tokens 4096
memory signal-density "fix checkout bug"
memory batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
memory runtime-profile list
memory runtime-plan "fix checkout bug" --runtime llama.cpp
memory cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
memory trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
```

## Inference Cost Stack

`memory doctor "<task>" --provider <provider>` includes an `Inference Cost Stack` section:

```text
Inference Cost Stack
raw_context_tokens: 12000
compiled_context_tokens: 1420
fresh_suffix_tokens: 260
cacheable_prefix_tokens: 880
omitted_tokens: 10580
estimated_prefill_reduction_percent: 88.2
estimated_kv_positions_avoided: 11740
signal_density_score: 0.98
duplicate_context_tokens_blocked: 3600
stale_context_tokens_blocked: 900
tool_trace_tokens_compressed: 6200
provider_cache_strategy: OpenAI cache plan
runtime_strategy: generic: reuse stable prefix; put fresh suffix last; warning: memory.cpp does not implement low-level kernels by default
```

## Cache audit

Cache hits often fail because stable and dynamic material are mixed together.

```bash
memory cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
```

Output shape:

```text
CACHE AUDIT
Provider: openai
Cache hit risk: high
Problems:
- dynamic text appears before stable/cacheable prefix
- timestamp-like text inside stable prefix
Fixes:
- move stable repo memory and rules before fresh request text
Stable prefix hash: ...
```

## Runtime profiles

Runtime profiles are advisory only. They do not enable kernels, KV quantization, speculative decoding, or batching by themselves.

```bash
memory runtime-profile list
memory runtime-plan "fix checkout bug" --runtime generic
memory runtime-plan "fix checkout bug" --runtime llama.cpp
memory runtime-plan "fix checkout bug" --runtime ollama
memory runtime-plan "fix checkout bug" --runtime vllm
memory runtime-plan "fix checkout bug" --runtime sglang
```

Each plan includes:

- recommended context budget
- prefix reuse hint
- KV quantization hint
- speculative decoding hint
- batching hint
- dynamic suffix placement
- warning that memory.cpp does not implement low-level kernels by default

## Fixtures

Small deterministic fixtures live in `tests/fixtures/inference/`:

- `huge_prompt_with_duplicates.txt`
- `stale_memories.json`
- `agent_trace_long.json`
- `multi_request_batch.json`
- `provider_cache_bad_order.md`
- `provider_cache_good_order.md`
- `runtime_profiles.json`
- `kv_budget_case.json`

They support tests for token reduction, stale exclusion, trace compression, batch grouping, cache audit, and runtime hints.
