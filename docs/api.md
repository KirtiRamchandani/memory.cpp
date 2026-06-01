# API surface

The `memory-core` crate exposes a small local SDK surface for apps that want the
same context-control reports used by the CLI. The API is deterministic,
local-only, and does not call providers by default.

| Function | Status | CLI equivalent |
| --- | --- | --- |
| `compileContext(options)` | available | `memory compile` |
| `createContextPack(options)` | available | `memory pack` |
| `doctor(options)` | available | `memory doctor` |
| `estimatePrefill(options)` | available | `memory prefill-report` |
| `estimateKvPressure(options)` | available | `memory kv-report` |
| `calculateSignalDensity(options)` | available | `memory signal-density` |
| `planProviderCache(options)` | available | `memory cache-plan` |
| `auditProviderCache(options)` | available | `memory cache-audit` |
| `compressToolTrace(options)` | available | `memory trace compress` |
| `rollupTrace(options)` | available | `memory trace-rollup` |
| `recordMemory(options)` | available | `memory remember` |
| `recordMistake(options)` | available | `memory mistake` |
| `attachProvider(options)` | available | `memory attach` |
| `generateRuntimePlan(options)` | available | `memory runtime-plan` |
| `generateBatchPlan(options)` | available | `memory batch-plan` |
| `askMemory(options)` | available | `memory ask` |
| `testMemory(options)` | available | `memory test` |
| `scoreAgentReadiness(options)` | available | `memory agents-score` |

Return shapes include compiled prompt, stable prefix, fresh suffix, context pack,
cache plan, KV report, prefill report, signal density, token firewall report,
warnings, evidence, omitted context, and files written.

## Minimal Rust example

```rust
use memory_core::{compileContext, ContextControlOptions};

let report = compileContext(ContextControlOptions {
    task: "fix billing export".to_string(),
    provider: "codex".to_string(),
    budget: 1500,
    context: vec![
        "Billing exports must preserve CSV column order.".to_string(),
        "Billing exports must preserve CSV column order.".to_string(),
    ],
});

assert!(report.kv_report.estimated_kv_positions_avoided > 0);
```

## Notes

- Estimates are approximate unless connected to real provider or runtime metrics.
- `memory.cpp` reduces KV pressure by reducing unnecessary tokens before inference.
- The SDK does not upload data or call provider APIs by default.
