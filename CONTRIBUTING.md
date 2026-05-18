# Contributing

Thanks for helping build `memory.cpp`.

## Local Setup

Install Rust from `https://rustup.rs/`, then run:

```bash
cargo fmt --all
cargo test --workspace
```

On Windows, you can use:

```powershell
.\scripts\verify.ps1
```

## Project Taste

This project should stay small, local-first, and boring to embed. Prefer:

- fewer runtime services
- explicit APIs
- measurable performance wins
- simple storage formats
- low memory overhead

Avoid adding broad frameworks unless they remove more complexity than they introduce.

## Pull Requests

Good pull requests include:

- a focused change
- tests for behavior changes
- benchmark notes for performance-sensitive paths
- docs updates when public APIs change
