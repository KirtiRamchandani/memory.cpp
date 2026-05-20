# Quickstart

`memory.cpp` helps your repo remember what happened, why it changed, and what to do next.

This page gets you from a fresh checkout to a local project map and daily recap in five minutes.

## 1. Install

From the repo root:

```bash
./scripts/install.sh
```

On Windows PowerShell:

```powershell
./scripts/install.ps1
```

If you do not want to install globally yet, replace `memory` with `cargo run -p memory-cli --` in every command below.

## 2. Let memory.cpp introduce itself

```bash
memory welcome
memory setup --developer
```

What happens:

- `.memory.cpp/` is created in this repo.
- `.memory.cpp/memory.db` becomes the local SQLite memory file.
- `.memoryignore` is created if it does not exist.
- A workspace is created for the repo.
- The setup prints detected tools such as Git, README, docs, CI config, package manager, Cursor, Claude, VS Code, and Ollama.

For a guided prompt-by-prompt flow:

```bash
memory setup --interactive
```

For a private/offline setup:

```bash
memory setup --private --offline
```

## 3. Check where data lives

```bash
memory what
memory where
memory privacy status
```

Expected shape:

```text
What memory.cpp does
memory.cpp helps your repo remember what happened, why it changed, and what to do next.

Where memory.cpp keeps data
Database: .memory.cpp/memory.db
Config:   .memory.cpp/memory-config.json
```

Nothing is uploaded to a cloud service by these commands.

## 4. Seed the demo

```bash
memory demo seed --workspace demo --path .
memory show-map --workspace demo --save .memory.cpp/demo/evolution.html
```

What this does:

- stores sample decisions, bugs, fixes, workflow notes, and launch tasks
- queues a sample candidate for review
- generates a self-contained HTML project evolution map

## 5. Try the daily developer loop

```bash
memory dev morning --workspace demo
memory dev resume "MCP integration" --workspace demo
memory dev explain-repo . --workspace demo
memory next --workspace demo
```

These commands answer:

- What was I doing?
- What changed recently?
- What broke?
- What did I plan to do next?
- What should I work on now?

## 6. Review candidates

```bash
memory show-inbox --workspace demo
memory inbox stats --workspace demo
memory inbox explain <candidate-id>
```

A candidate is a suggested memory that needs human approval before becoming durable memory.

## 7. Create AI assistant context

```bash
memory show-context --workspace demo
memory dev context --for cursor --workspace demo
memory dev context --for codex --workspace demo
memory dev context --for claude --workspace demo
```

The output is a clean context block with repo summary, recent decisions, important files, commands to run, known pitfalls, and source citations.

## 8. Diagnose setup

```bash
memory doctor --workspace demo
```

`doctor` checks database health, schema readability, workspace status, MCP safety defaults, Cursor/Claude/Ollama signals, proxy port availability, Git status, map export support, runtime state, and smoke-test hints.

## 9. Delete everything if you want a clean slate

```bash
memory privacy purge --yes
```

Or remove the local folder yourself:

```bash
rm -rf .memory.cpp
```

PowerShell:

```powershell
Remove-Item -Recurse -Force .memory.cpp
```

## What just happened?

You created a local memory store, reviewed where it lives, generated a project map, asked for a daily recap, and produced AI assistant context. The repo now has a small local memory layer without requiring a hosted service.
