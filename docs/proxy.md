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
memory --db .memory.cpp/memory.db proxy --listen 127.0.0.1:7332 --upstream http://127.0.0.1:11434 --workspace demo
```

Then point any OpenAI-compatible client at:

```text
http://127.0.0.1:7332/v1
```

## Attach Ollama helper

```bash
memory --db .memory.cpp/memory.db attach ollama --workspace demo --start-proxy
```

This writes a small local proxy config file and can start the proxy immediately.

## Good launch demo

```bash
memory init --workspace demo
memory demo seed --workspace demo --path .
memory attach ollama --workspace demo --start-proxy
```

Then ask the model what your project stack is and follow it with `memory explain` or `memory dev morning`.
