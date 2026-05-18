# Embeddings

`memory.cpp` keeps embeddings practical and local-first.

## Providers available today

- `hash`
- `ollama`
- `openai`

## Why the default is `hash`

The hash embedder has almost zero setup friction, which makes the launchable core much easier to adopt and demo.

That is a deliberate v0.2.1 tradeoff.

## Provider selection

```bash
memory --db .memory.cpp/memory.db --embedder hash init --workspace demo
memory --db .memory.cpp/memory.db --embedder ollama --endpoint http://127.0.0.1:11434 init --workspace demo
memory --db .memory.cpp/memory.db --embedder openai --endpoint https://api.openai.com init --workspace demo
```

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
