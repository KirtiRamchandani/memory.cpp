# ADR 0001: SQLite As The Core Store

## Status
Accepted

## Decision
Use SQLite as the core memory store for `memory.cpp`.

## Why
- local-first by default
- portable single-file storage
- simple backup and restore path
- easy snapshots and version history
- good fit for solo-developer workflows

## Consequences
- the product can honestly position itself as a durable local memory primitive
- future encryption, sync, and export work can build on a stable file-based core
