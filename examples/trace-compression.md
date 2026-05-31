# Tool trace compression example

Command:

```bash
memory trace compress --file examples/agent-log.txt
```

Output shape:

```text
tool_trace_summary:
  goal: infer from surrounding task or prompt
  attempted:
    - $ cargo test -p memory-cli
  failed_attempts:
    - error: failed in crates/memory-cli/src/main.rs
  final_error: error: failed in crates/memory-cli/src/main.rs
  useful_findings:
    - fix: replace stale helper call and rerun clippy
  files_touched:
    - crates/memory-cli/src/main.rs
  next_best_action: rerun the smallest failing command after applying the remembered fix
  token_original: 420
  token_summary: 96
```

What just happened: a noisy agent/tool log became a small local summary suitable for future memory.
