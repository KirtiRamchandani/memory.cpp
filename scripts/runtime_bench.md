# Runtime benchmark (measured TTFT, not estimated)

CAMC-Bench and `kv-sim` report *estimated* token and KV savings. To turn
estimates into **measured** evidence, run `scripts/runtime_bench.py` against a
real OpenAI-compatible local runtime. It measures actual time-to-first-token
(TTFT) for a raw vs compiled prompt, and the benefit of a byte-stable prefix on
a repeated (warm) call where prefix caching applies.

## 1. Start a runtime

```bash
# vLLM (prefix caching is the point — keep it on)
vllm serve meta-llama/Llama-3.1-8B-Instruct --enable-prefix-caching      # :8000

# or llama.cpp
llama-server -m model.gguf -np 4 --port 8080                              # :8080

# or Ollama (OpenAI-compatible at /v1)
ollama serve                                                             # :11434

# or SGLang
python -m sglang.launch_server --model meta-llama/Llama-3.1-8B-Instruct   # :30000
```

## 2. Measure

```bash
python scripts/runtime_bench.py \
  --base-url http://localhost:8000/v1 \
  --model meta-llama/Llama-3.1-8B-Instruct \
  --task "fix the billing export bug"
```

## 3. Read the result

You get TTFT for `RAW` vs `COMPILED`, each measured cold and warm:

```
MEASURED time-to-first-token (lower is better):
  RAW        cold   ###.# ms   warm   ###.# ms   warm speedup  ##.#%
  COMPILED   cold   ###.# ms   warm   ###.# ms   warm speedup  ##.#%
```

A compiled prompt is shorter (less prefill) and its stable prefix is byte-stable
across tasks, so a prefix-caching runtime can reuse it on the warm call. Report
these numbers **separately** from the estimates — they are real measurements of
your stack, and they are the screenshot worth sharing.

> No GPU is used by the harness itself; it only times whatever endpoint you point
> it at.
