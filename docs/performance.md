# Performance

`memory.cpp` should stay small, local, and fast enough to use every day.

## Current performance posture

- SQLite is the default durable store.
- The default embedding path is lightweight hashing for low-RAM and offline use.
- Optional network/model providers are opt-in.
- Maps, context packs, and share artifacts are generated on demand.
- Git/terminal/CI watchers should create candidates without requiring heavy background services.

## Benchmark commands

Run the core recall benchmark:

```bash
cargo bench -p memory-core --bench recall
```

Run a release build smoke test:

```bash
cargo build --release -p memory-cli
./target/release/memory release-check
```

Windows:

```powershell
cargo build --release -p memory-cli
./target/release/memory.exe release-check
```

## Benchmarks to track before 1.0

| Area | Target question |
| --- | --- |
| Recall latency | How fast is a normal search over a local repo memory DB? |
| Context generation | How long does `memory context write` take for small/medium repos? |
| Map generation | How long does HTML map export take? |
| Candidate inbox | How many candidates can be reviewed/listed without sluggish output? |
| Startup time | How fast does `memory --help` and `memory status` return? |
| Database size | How does `.memory.cpp/memory.db` grow over time? |
| Low-RAM mode | Does hash retrieval stay usable on small machines? |

## Reporting format

When publishing benchmark numbers, include:

- commit SHA
- OS and CPU
- Rust version
- command used
- database size and memory count
- mean/median latency when available
- whether the build was debug or release

Do not publish performance claims without this context.