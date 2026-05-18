# Proxy

`memory proxy` is the quickest way to make a local chat client feel persistent.

## How it works

The proxy sits between an OpenAI-compatible client and an upstream model runtime.

Flow:

1. incoming chat request
2. extract the user query
3. recall relevant memory
4. inject a local memory block into the prompt
5. forward the request upstream
6. observe the response
7. queue candidate memories for review when the response contains durable facts

## Run the proxy

```bash
memory --db .memory.cpp/memory.db proxy --listen 127.0.0.1:7332 --upstream http://127.0.0.1:11434 --workspace demo --learn --approval-required
```

Then point any OpenAI-compatible client at:

```text
http://127.0.0.1:7332/v1
```

## Learning modes

The proxy can now observe model replies and turn durable engineering facts into memory candidates.

Safe launch mode:

```bash
memory --db .memory.cpp/memory.db proxy --listen 127.0.0.1:7332 --upstream http://127.0.0.1:11434 --workspace demo --learn --approval-required
```

Useful flags:

- `--learn`: enable response observation and memory extraction
- `--approval-required`: queue extracted memories for review instead of auto-storing them
- `--min-confidence 0.70`: ignore weaker candidate extractions
- `--dry-run`: print extracted candidates without storing anything

## Attach Ollama helper

```bash
memory --db .memory.cpp/memory.db attach ollama --workspace demo --start-proxy
```

This writes a small local proxy config file and can start the proxy immediately.

When `--start-proxy` is used, the helper starts the proxy in safe learning mode so chat-derived memories land in the review path first.

## Good launch demo

```bash
memory init --workspace demo
memory demo seed --workspace demo --path .
memory attach ollama --workspace demo --start-proxy
```

Then ask the model what your project stack is and follow it with `memory explain` or `memory dev morning`.
