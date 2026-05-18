# ADR 0003: Map As A First-Class Product Surface

## Status
Accepted

## Decision
Treat `memory map` as a core launch surface rather than a secondary visualization feature.

## Why
- most memory systems are invisible
- a map produces instant visual payoff
- project evolution, decisions, bugs, and architecture are easier to explain visually
- the output is naturally shareable in demos, docs, and onboarding flows

## Consequences
- `memory demo seed` generates map artifacts automatically
- HTML, Mermaid, Markdown, and JSON outputs are part of the v0.2.1 core
- future domain packs can extend the same map engine instead of inventing new visual systems
