# Known limitations

`memory.cpp` is useful today, but it is not magic. These limitations are intentional and documented.

## Memory quality depends on signals

If a repo has no memories, no Git history, no terminal data, and no CI logs, commands such as `memory dev morning` will say that data is missing. Seed useful memory with:

```bash
memory setup --developer --yes
memory demo seed --path .
memory inbox review
```

## It does not infer every dependency automatically

`memory.cpp` can summarize files, commands, maps, and known decisions, but it does not fully understand every code dependency unless that information is captured or imported.

## It does not replace Git

Git remains the source of truth for diffs and commits. `memory.cpp` adds context, rationale, and recall on top of Git.

## It does not replace docs

Generated docs and handoff bundles are starting points. Review them before publishing.

## It does not guarantee distributed exactly-once behavior

Local memory, CI import, Git watch, and terminal memory are practical developer tools. Distributed sync and exactly-once delivery are out of scope.

## Some integrations are beta

Editor and assistant attach flows are dry-run first and config-safe, but tool-specific config formats can change.

## Devtools/dashboard are local and experimental

The dashboard is not a hosted product. Do not expose local devtools to untrusted networks.

## FastEmbed/ONNX wording is intentionally cautious

The current provider label represents local semantic retrieval intent. The repo does not bundle a true ONNX Runtime backend today.

## Experimental commands may change

Terminal memory, CI memory, dashboard surfaces, and some map exports are experimental. Use `memory release-check` and the docs before relying on them in automation.