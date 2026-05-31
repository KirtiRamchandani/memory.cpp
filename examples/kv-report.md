# KV pressure report example

Command:

```bash
memory kv-report "summarize customer history"
```

Output shape:

```text
KV PRESSURE REPORT
Task: summarize customer history
Raw context tokens: 18320
Compiled context tokens: 1240
Cacheable prefix tokens: 880
Fresh suffix tokens: 260
Omitted tokens: 17080
Estimated KV pressure avoided: 17080 token positions
Estimated context reduction: 93.2%
Runtime notes:
- memory.cpp reduces KV pressure by preventing unnecessary tokens from entering the model.
- Estimated KV numbers are approximate token-position savings, not exact speedups.
- Runtime KV quantization, prefix reuse, batching, and speculative decoding remain separate optional runtime features where supported.
```

What just happened: memory.cpp estimated prompt-side KV pressure avoided by sending less context.
