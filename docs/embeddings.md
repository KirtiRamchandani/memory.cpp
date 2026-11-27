# Embeddings

`memory.cpp` keeps embeddings practical and local-first.

## Providers available today

- `hash`
- `ollama`
- `openai`
- `aimlapi` (opt-in)

## Why the default is `hash`

The hash embedder has almost zero setup friction, which makes the launchable core much easier to adopt and demo.

That is a deliberate v0.2.1 tradeoff.

## Provider selection

```bash
memory --db .memory.cpp/memory.db --embedder hash init --workspace demo
memory --db .memory.cpp/memory.db --embedder ollama --endpoint http://127.0.0.1:11434 init --workspace demo
memory --db .memory.cpp/memory.db --embedder openai --endpoint https://api.openai.com init --workspace demo
AIMLAPI_API_KEY=... memory --db .memory.cpp/memory.db --embedder aimlapi --dimensions 1536 init --workspace demo
```

## AI/ML API (opt-in)

`--embedder aimlapi` sends embedding requests to `https://api.aimlapi.com/v1/embeddings` with the key from `AIMLAPI_API_KEY`. The default model is `openai/text-embedding-3-small` (1536 dimensions, so pass `--dimensions 1536`); override it with `--model`, for example `openai/text-embedding-3-large`. The host is fixed: `--endpoint` is ignored, the key is only sent to `api.aimlapi.com`, and redirects are not followed. Nothing changes unless you select it; `hash` stays the default.

## What `doctor` checks

`memory doctor` validates:

- selected embedder
- configured dimension count
- Ollama reachability when relevant
- local safety defaults around the rest of the stack

## What is intentionally later

The backlog still includes:

- provider registry polish
- migration helpers
- FastEmbed / ONNX backends
- richer model-specific retrieval modes

Those belong after the generic developer experience is excellent.
