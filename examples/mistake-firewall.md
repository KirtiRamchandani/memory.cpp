# Mistake firewall example

Command:

```bash
memory mistake "Use cargo fmt before committing Rust changes."
memory mistakes
```

Output shape:

```text
mistake rule stored: mem_01J...
included automatically in relevant context packs.

mistake firewall rules
- mem_01J... Mistake firewall rule: Use cargo fmt before committing Rust changes.
```

What just happened: a hard rule was stored as a local workflow memory and can be included in future context packs.
