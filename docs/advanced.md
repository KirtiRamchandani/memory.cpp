# Advanced usage

Advanced memory.cpp workflows are still local-first and deterministic.

## Control-plane commands

- `memory explain-compile "<task>"`
- `memory trust-report`
- `memory flight start --goal "<goal>" --tool codex`
- `memory context-diff latest previous`
- `memory test --file memory.tests.yaml`
- `memory ci-check`
- `memory shared-context export`
- `memory agents-score --for codex`
- `memory roi --input-cost <num>`

## Safety note

These commands do not upload data, start cloud services, or implement low-level inference kernels.
