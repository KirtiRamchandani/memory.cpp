# Roadmap

`memory.cpp` is not done yet. The current goal is a polished, credible developer tool that people can install, understand, and use daily.

The product promise stays narrow:

> Your repo remembers.

## Current public developer adoption release

Focus:

- one-command setup and install clarity
- daily developer commands
- AI context packs
- Git, terminal, and CI memory basics
- candidate inbox trust
- project maps and timelines
- shareable PR, handoff, release, and onboarding artifacts
- local-first privacy and redaction
- release hardening, compatibility docs, and security policy

## v0.x: Hardening before expansion

Before adding major surfaces, improve:

- fresh-clone release-candidate validation
- built-binary smoke tests
- cross-platform install verification
- map/context output snapshots
- redaction fixtures
- corrupt database recovery tests
- Windows path tests
- benchmark reporting
- C API stability notes
- release archive verification

## Daily developer habit backlog

- better `memory dev morning` source citations
- better `memory dev resume <topic>` relevance
- richer terminal command recall after opt-in
- better CI failure grouping
- more useful `memory dev readme-suggest`
- more practical `memory pr checklist`
- clearer stale decision detection
- more map compare/export options

## Trust and safety backlog

- stronger `.memoryignore` diagnostics
- more redaction fixtures
- private-safe artifact audit output
- MCP write-policy receipts
- backup/restore polish
- optional encrypted local store research
- clearer integration config repair flow

## Packaging backlog

- release artifact install verification
- macOS Apple Silicon artifact naming
- Linux ARM/Raspberry Pi notes
- Homebrew formula exploration
- Windows PATH troubleshooting improvements
- signed release policy research

## Deferred on purpose

Do not rush these before the developer core is loved:

- hosted SaaS
- enterprise team sync
- billing
- plugin marketplace
- mobile packs
- webapp/AppSec packs
- fuzzing packs
- heavyweight browser extension
- Electron desktop app
- complex permission systems
- distributed memory server

## v1.0 bar

A v1.0 candidate should have:

- stable core CLI and storage compatibility promises
- documented C API stability
- release-candidate script passing from a fresh clone
- Linux, macOS, and Windows CI passing
- install docs tested on all supported platforms
- security policy and known limitations kept current
- examples that run or are clearly marked static
- no false maturity claims for beta/experimental surfaces