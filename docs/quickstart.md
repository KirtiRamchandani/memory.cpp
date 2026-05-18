# Quickstart

This guide takes you from an empty local database to a shareable project map in a few minutes.

## 1. Install

From the repo root:

```bash
./scripts/install.sh
```

On Windows PowerShell:

```powershell
./scripts/install.ps1
```

If you prefer not to install globally yet, replace `memory` with `cargo run -p memory-cli --` in every command below.

## 2. Initialize a workspace

```bash
memory --db .memory.cpp/memory.db init --workspace demo
```

This creates the local SQLite-backed memory file and sets `demo` as the default workspace.

Optional but recommended before watching or importing a real repo:

```text
.memoryignore
```

Example:

```text
.env
*.pem
*.key
secrets/
node_modules/
target/
```

## 3. Seed the launch demo

```bash
memory --db .memory.cpp/memory.db demo seed --workspace demo --path .
```

What this does:

- creates or activates the `demo` workspace
- stores a realistic set of decisions, bugs, fixes, workflow notes, and launch tasks
- queues a candidate memory for review
- writes shareable map artifacts under `.memory.cpp/demo/`

Expected outputs:

- `.memory.cpp/demo/evolution.html`
- `.memory.cpp/demo/evolution.mmd`
- `.memory.cpp/demo/decisions.md`
- `.memory.cpp/demo/architecture.mmd`

## 4. Generate a project evolution map

```bash
memory --db .memory.cpp/memory.db map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
```

Open the generated HTML file in a browser. The export is self-contained and includes:

- search
- class filter
- date filters
- citations
- notes
- edges

## 5. Try the daily workflow surface

```bash
memory --db .memory.cpp/memory.db dev morning --workspace demo
memory --db .memory.cpp/memory.db dev resume "MCP integration" --workspace demo
```

`dev morning` is the best quick proof that the repo can explain itself.

## 6. Attach a coding agent

```bash
memory --db .memory.cpp/memory.db attach cursor --workspace demo
```

This writes a local MCP config that points Cursor at `memory.cpp` in read-only mode by default.

Other supported targets:

```bash
memory --db .memory.cpp/memory.db attach codex --workspace demo
memory --db .memory.cpp/memory.db attach claude --workspace demo
memory --db .memory.cpp/memory.db attach vscode --workspace demo
```

## 7. Start the local dashboard/runtime

```bash
memory --db .memory.cpp/memory.db start --workspace demo
memory --db .memory.cpp/memory.db status
```

This starts the local dashboard/API in the background and writes runtime state under `.memory.cpp/runtime/`.

Stop it when you are done:

```bash
memory --db .memory.cpp/memory.db stop
```

## 8. Validate the setup

```bash
memory --db .memory.cpp/memory.db doctor --workspace demo
```

`doctor` checks:

- database availability
- schema readability
- active workspace
- git detection
- MCP safety defaults
- Ollama reachability
- export directory writability
- runtime state
- API port availability

## 8.5. Inspect agent access receipts

```bash
memory --db .memory.cpp/memory.db audit-log --limit 10
```

This reads the local MCP access log so you can verify which tools were used and whether any operation was blocked.

## 9. Try the proxy demo

If Ollama is running locally:

```bash
memory --db .memory.cpp/memory.db attach ollama --workspace demo --start-proxy
```

Or run the proxy directly:

```bash
memory --db .memory.cpp/memory.db proxy --listen 127.0.0.1:7332 --upstream http://127.0.0.1:11434 --workspace demo
```

Then point any OpenAI-compatible client at:

```text
http://127.0.0.1:7332/v1
```

## 10. Run the smoke test

```bash
./scripts/smoke.sh
```

On Windows PowerShell:

```powershell
./scripts/smoke.ps1
```

The smoke script covers init, demo seed, map export, doctor, runtime start/stop, MCP tool listing, and audit-log visibility.
