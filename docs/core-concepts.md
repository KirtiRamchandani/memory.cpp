# Core concepts

## What is a memory?

A memory is a useful project note that should survive context switches.

Examples:

- Decision: use SQLite because the store must stay portable.
- Bug fix: ECONNRESET was fixed by restarting the local test DB.
- Workflow: run cargo test before opening a PR.

## What is a workspace?

A workspace is a named scope. Most people use one workspace per repo.

## What is a candidate?

A candidate is a possible memory waiting for review.

```bash
memory inbox
memory inbox explain <id>
memory inbox approve <id>
```

## What is provenance?

Provenance means where this came from: file, line, commit, terminal command, CI log, or import.

## What is a context pack?

A context pack is a clean summary for an AI assistant.

```bash
memory dev context --for cursor
```

## What is a map?

A map is a visual explanation of project evolution, decisions, bugs, and impact.

```bash
memory map why "SQLite storage"
```

## What is local-first?

Your memory database lives on your machine. The default path is .memory.cpp/memory.db.

## What does MCP mean?

MCP is a local protocol that lets tools ask memory.cpp for context. In memory.cpp it is read-only by default.

## What does proxy mean?

The proxy is a local OpenAI-compatible endpoint that can add memory context to model requests.

## What is an embedding?

An embedding helps search find related text. memory.cpp has local defaults and optional provider settings.
