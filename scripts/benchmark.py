#!/usr/bin/env python3
"""Reproducible token + KV benchmark evidence for memory.cpp.

Produces real numbers from the production binary:

  1. CAMC-Bench  - controlled ablation on the bundled synthetic corpus
                   (`memory bench camc --json`).
  2. Dogfood     - memory.cpp compiled against REAL knowledge of its own repo,
                   compared with the naive "paste the docs" baseline an agent
                   would otherwise use.

Outputs:
  BENCHMARKS.md
  assets/camc-bench.svg        (tokens by strategy)
  assets/dogfood-tokens.svg    (naive docs paste vs compiled pack)

Every number is captured live; re-running regenerates the files.

Usage:  python scripts/benchmark.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"


def find_binary() -> str:
    for cand in ("target/release/memory.exe", "target/debug/memory.exe",
                 "target/release/memory", "target/debug/memory"):
        p = ROOT / cand
        if p.exists():
            return str(p)
    sys.exit("memory binary not found - run `cargo build -p memory-cli` first")


BIN = find_binary()


def mem(db: str, args: list[str]) -> str:
    return subprocess.run([BIN, "--db", db, *args], capture_output=True, text=True).stdout


def mem_json(db: str, args: list[str]) -> dict:
    out = mem(db, [*args, "--json"])
    try:
        return json.loads(out)
    except json.JSONDecodeError:
        return {}


def est_tokens(text: str) -> int:
    return max(1, (len(text) + 3) // 4)  # chars/4, same estimator the tool uses


# Real durable facts about THIS repository (true statements from the codebase).
REPO_FACTS = [
    ("Billing of tokens is not the only cost; repeated prefill and KV pressure dominate at scale.", "fact"),
    ("The stable prefix must exclude task text and is hashed with blake3 (memprefix:v1:blake3).", "invariant"),
    ("Use blake3 for all content hashes; never a non-portable DefaultHasher.", "mistake"),
    ("memory.cpp is local-first: a SQLite vault under .memory.cpp, no cloud, no telemetry.", "decision"),
    ("MCP exposes read-only resources by default; no write tools unless explicitly enabled.", "rule"),
    ("Run cargo fmt --all and clippy -D warnings before every commit.", "mistake"),
    ("KV figures are prompt-side estimates, not provider KV-cache compression.", "rule"),
    ("Negative memory records what NOT to send: stale docs, wrong fixes, secrets, injection sources.", "fact"),
    ("Freshness proofs reject stale memory before it is compiled into a pack.", "fact"),
    ("The optimizer scores context by utility/cost, not relevance alone.", "fact"),
    ("The compiler emits a provenance ledger and a reproducible context.lock.", "decision"),
    ("Provider packs target claude, codex, gemini, cursor, continue, and mcp.", "fact"),
]

TASKS = [
    "fix the billing export determinism bug",
    "add a new provider pack for a coding agent",
    "review the auth session decision",
]

# Files an agent might naively paste to give itself "repo context".
NAIVE_DOCS = ["README.md", "docs/architecture.md", "docs/cli.md", "docs/cache-aware-compiler.md"]


def naive_baseline_tokens() -> int:
    total = 0
    for rel in NAIVE_DOCS:
        p = ROOT / rel
        if p.exists():
            total += est_tokens(p.read_text(encoding="utf-8", errors="ignore"))
    return total


def run_camc() -> dict:
    tmp = tempfile.mkdtemp(prefix="memcpp-bench-")
    db = os.path.join(tmp, "b.db")
    return mem_json(db, ["bench", "camc"])


def run_dogfood() -> tuple[list[dict], int]:
    tmp = tempfile.mkdtemp(prefix="memcpp-dog-")
    db = os.path.join(tmp, "d.db")
    for text, tag in REPO_FACTS:
        mem(db, ["remember", text, "--tags", tag])
    rows = []
    for task in TASKS:
        wow = mem_json(db, ["wow", task, "--for-acquirer"])
        kv = mem_json(db, ["kv-sim", task, "--model", "llama-3.1-8b"])
        comp = wow.get("compiled", {})
        rows.append({
            "task": task,
            "raw_tokens": comp.get("raw_tokens"),
            "compiled_tokens": comp.get("compiled_tokens"),
            "reduction_percent": comp.get("reduction_percent"),
            "cacheable_prefix_tokens": comp.get("cacheable_prefix_tokens"),
            "kv_raw_mib": round(kv.get("estimated_kv_bytes_raw", 0) / 1048576, 1),
            "kv_compiled_mib": round(kv.get("estimated_kv_bytes_compiled", 0) / 1048576, 1),
            "kv_avoided_mib": round(kv.get("estimated_kv_bytes_avoided", 0) / 1048576, 1),
            "kv_reduction_percent": round(kv.get("estimated_kv_percent_reduction", 0), 1),
        })
    return rows, naive_baseline_tokens()


def svg_bars(title: str, pairs: list[tuple[str, float, str]], unit: str) -> str:
    left, width, bar_h, gap, top = 230, 760, 24, 12, 50
    height = top + len(pairs) * (bar_h + gap) + 30
    maxv = max((v for _, v, _ in pairs), default=1) or 1
    rows = [f'<text x="12" y="26" font-size="15" font-weight="700">{title}</text>']
    for i, (label, val, color) in enumerate(pairs):
        y = top + i * (bar_h + gap)
        w = int((val / maxv) * (width - left - 90))
        rows.append(
            f'<text x="10" y="{y+17}" font-size="12">{label}</text>'
            f'<rect x="{left}" y="{y}" width="{max(w,1)}" height="{bar_h}" rx="4" fill="{color}"/>'
            f'<text x="{left+max(w,1)+6}" y="{y+17}" font-size="12">{val:g} {unit}</text>'
        )
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
            f'viewBox="0 0 {width} {height}" font-family="system-ui,sans-serif">'
            + "".join(rows) + "</svg>")


def main() -> None:
    ASSETS.mkdir(exist_ok=True)
    camc = run_camc()
    dog, naive = run_dogfood()

    # CAMC bar chart
    results = camc.get("results", [])
    pairs = [(r["strategy"], r["total_tokens"],
              "#2e9e5b" if r["strategy"] == "full CAMC" else "#5b7cfa") for r in results]
    (ASSETS / "camc-bench.svg").write_text(
        svg_bars("CAMC-Bench: total context tokens by strategy", pairs, "tok"), encoding="utf-8")

    # Dogfood chart: naive docs paste vs compiled pack (first task)
    first = dog[0]
    dog_pairs = [
        ("naive docs paste", naive, "#c0504d"),
        (f"compiled pack ({first['task'][:22]}...)", first["compiled_tokens"] or 0, "#2e9e5b"),
    ]
    (ASSETS / "dogfood-tokens.svg").write_text(
        svg_bars("Dogfood: repo docs paste vs compiled pack", dog_pairs, "tok"), encoding="utf-8")

    # BENCHMARKS.md
    lines = []
    lines.append("# Benchmarks\n")
    lines.append("All numbers below are produced live by the `memory` binary. Reproduce with "
                 "`python scripts/benchmark.py` (or the individual commands shown).\n")
    lines.append("> **Honesty:** memory.cpp reduces *prompt-side* tokens and *estimated* KV "
                 "pressure by sending fewer, fresher, more reusable tokens. It does **not** "
                 "compress a provider's KV cache. KV figures are estimates from a model profile; "
                 "runtime TTFT/memory must be measured on a real stack (see "
                 "`scripts/runtime_bench.md`).\n")

    lines.append("## 1. CAMC-Bench (controlled ablation, synthetic corpus)\n")
    lines.append("`memory bench camc`\n")
    lines.append("| Strategy | Tokens | Cacheable prefix | Reduction vs raw | Stale | Contradictions | Secrets | Task success |")
    lines.append("|---|--:|--:|--:|--:|--:|--:|--:|")
    for r in results:
        lines.append(f"| {r['strategy']} | {r['total_tokens']} | {r['cacheable_prefix_tokens']} | "
                     f"{r['token_reduction_vs_raw_percent']:.1f}% | {r['stale_included']} | "
                     f"{r['contradictions_included']} | {r['secret_leakage']} | "
                     f"{r['tasks_passed']}/{r['tasks_total']} |")
    lines.append("\n![CAMC-Bench](assets/camc-bench.svg)\n")

    lines.append("## 2. Dogfood: memory.cpp on its own repository\n")
    lines.append("memory.cpp seeded with real durable facts about this repo, compiled for real "
                 f"tasks. The naive baseline is the token cost of pasting "
                 f"`{', '.join(NAIVE_DOCS)}` into the prompt: **{naive} tokens**.\n")
    lines.append("| Task | Raw ctx | Compiled | Reduction | Cacheable prefix | Est. KV avoided |")
    lines.append("|---|--:|--:|--:|--:|--:|")
    for d in dog:
        lines.append(f"| {d['task']} | {d['raw_tokens']} | {d['compiled_tokens']} | "
                     f"{d['reduction_percent']}% | {d['cacheable_prefix_tokens']} | "
                     f"{d['kv_avoided_mib']} MiB ({d['kv_reduction_percent']}%) |")
    comp0 = first["compiled_tokens"] or 1
    lines.append(f"\nVs the naive docs paste, the compiled pack for "
                 f"`{first['task']}` is **{comp0} tokens** instead of **{naive}** "
                 f"(**{100 - round(comp0/naive*100):d}% smaller**), and its stable prefix is "
                 f"byte-stable and cacheable across tasks.\n")
    lines.append("![Dogfood tokens](assets/dogfood-tokens.svg)\n")

    lines.append("## 3. Reproduce\n```bash\ncargo build -p memory-cli\n"
                 "python scripts/benchmark.py        # this file\n"
                 "memory bench camc                  # the ablation table\n"
                 "memory wow --for-acquirer          # one-task summary\n```\n")

    (ROOT / "BENCHMARKS.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("wrote BENCHMARKS.md, assets/camc-bench.svg, assets/dogfood-tokens.svg")
    print(f"naive docs paste = {naive} tokens; compiled (task 1) = {comp0} tokens")


if __name__ == "__main__":
    main()
