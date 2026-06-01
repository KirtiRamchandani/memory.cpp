# API surface

The Rust crates are still pre-1.0. This document describes the intended stable surface for future SDK wrappers.

| Function | Status | CLI equivalent |
| --- | --- | --- |
| `compileContext(options)` | planned | `memory compile` |
| `createContextPack(options)` | planned | `memory pack` |
| `doctor(options)` | planned | `memory doctor` |
| `estimatePrefill(options)` | planned | `memory prefill-report` |
| `estimateKvPressure(options)` | planned | `memory kv-report` |
| `calculateSignalDensity(options)` | planned | `memory signal-density` |
| `planProviderCache(options)` | planned | `memory cache-plan` |
| `auditProviderCache(options)` | planned | `memory cache-audit` |
| `compressToolTrace(options)` | planned | `memory trace compress` |
| `rollupTrace(options)` | planned | `memory trace-rollup` |
| `recordMemory(options)` | planned | `memory remember` |
| `recordMistake(options)` | planned | `memory mistake` |
| `attachProvider(options)` | planned | `memory attach` |
| `generateRuntimePlan(options)` | planned | `memory runtime-plan` |
| `generateBatchPlan(options)` | planned | `memory batch-plan` |
| `askMemory(options)` | planned | `memory ask` |
| `testMemory(options)` | planned | `memory test` |
| `scoreAgentReadiness(options)` | planned | `memory agents-score` |

Return shapes should include compiled prompt, stable prefix, fresh suffix, cache plan, KV report, prefill report, warnings, evidence, omitted context, and files written.
