# Avoid repeating AI coding mistakes

Goal: make repeated mistakes visible to future context packs.

## Commands

```bash
memory mistake "Use cargo fmt before committing Rust changes."
memory mistake "Do not claim true ONNX Runtime unless it is bundled."
memory mistakes
memory doctor "prepare release notes" --provider gemini
```

## Expected output

```text
mistake firewall rules
- mem_... Mistake firewall rule: Use cargo fmt before committing Rust changes.
- mem_... Mistake firewall rule: Do not claim true ONNX Runtime unless it is bundled.
```

## What happened

The rules were stored as local workflow memories tagged as mistake rules. Future compiled context packs can include them when relevant.

## Privacy note

Mistake rules are local memories. Do not store secrets or private personal data as rules.

## Next step

```bash
memory compile "prepare release notes" --provider generic --budget 1500
```
