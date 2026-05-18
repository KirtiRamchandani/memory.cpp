# Troubleshooting

## `cargo` is not on PATH

If `cargo` is installed but the shell cannot find it, add the default Rust path first.

PowerShell:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

## Ollama is not reachable

`memory doctor` will warn if `http://localhost:11434` is unavailable.

If you are using the default hash embedder and not running the proxy yet, this warning is informational.

## The CLI uses a pre-parser for some commands

A small group of v0.2.1 commands currently use a manual pre-parser because an oversized nested Clap tree hit a stack-overflow edge case.

This is documented and covered with parsing tests. It is not ideal forever, but it is a deliberate launch tradeoff rather than a hidden bug.

## The map command prints HTML to the terminal

Use `--save` when generating files:

```bash
memory map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
```

## The repo has no git history yet

`memory map` still works without commits. It falls back to:

- memory events
- stored memory
- entity graph
- docs and README content

Git enrichment becomes additive once the repository has commits and tags.
