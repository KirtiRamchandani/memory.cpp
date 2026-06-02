# Demo script

Run this local demo without API keys:

```bash
memory init
memory demo seed --workspace demo
memory doctor "fix the billing export bug" --provider openai
memory pack "fix the billing export bug" --for codex --budget 1500
memory preflight --for codex "fix the billing export bug"
memory agents-score --for codex
memory bench
```

Expected result: token waste blocked, KV pressure estimated, provider cache plan generated, stale memory excluded, hard rules included, and static reports available under `.memory.cpp/`.

For the current launch evidence matrix, see [launch-audit.md](launch-audit.md).

## Deterministic terminal transcript

For README screenshots, launch posts, or a public walkthrough, generate a bounded terminal-only transcript:

```bash
./scripts/demo-terminal.sh
```

PowerShell:

```powershell
./scripts/demo-terminal.ps1
```

Dry-run mode writes the same artifact layout without executing the CLI:

```bash
./scripts/demo-terminal.sh --dry-run
```

```powershell
./scripts/demo-terminal.ps1 -DryRun
```

Artifacts are written under `.memory.cpp/reports/demo/` by default:

- `terminal-demo.txt`
- `terminal-demo.md`
- `recording-tools.md`
- `codex-pack.md`
- `ship-demo.md`

Optional VHS, asciinema, or agg recording tools are detected when present, but memory.cpp never installs them automatically.
