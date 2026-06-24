# CAMC paper

`camc.tex` — *Cache-Aware Memory Compilation for Token-Efficient Coding Agents*.

## Build

```bash
pdflatex camc.tex     # or: latexmk -pdf camc.tex
```

Only standard packages are used (`geometry`, `booktabs`, `hyperref`, `amsmath`,
`xcolor`, `enumitem`).

## Regenerate the data

Every number in the benchmark table comes from the production code paths and is
reproducible:

```bash
memory bench camc                 # the table, as text
memory paper tables               # the table, as markdown
memory paper run                  # tables.md, report.json, methodology.json, reduction.svg
```

The paper deliberately reports the synthetic-corpus result as a *mechanism* study.
Real-repo and runtime (vLLM / SGLang / llama.cpp / Ollama) numbers must be measured
live and reported separately from the prompt-side estimates.
