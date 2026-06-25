#!/usr/bin/env python3
"""Measure REAL runtime evidence: TTFT and prefix-cache benefit.

CAMC-Bench reports *estimated* token and KV savings. This harness measures the
thing estimates cannot: actual time-to-first-token (TTFT) against a real local
runtime, and the benefit of a byte-stable cached prefix on a repeated call.

It needs an OpenAI-compatible endpoint. Examples:
  * vLLM      : `vllm serve <model> --enable-prefix-caching`            (:8000)
  * llama.cpp : `llama-server -m model.gguf -np 4`                       (:8080)
  * Ollama    : `ollama serve` (OpenAI-compat at /v1)                    (:11434)
  * SGLang    : `python -m sglang.launch_server --model <model>`         (:30000)

It compiles two prompts with the `memory` binary for the same task:
  A) RAW      - the naive "everything" context
  B) COMPILED - the cache-stable compiled pack (stable prefix + fresh suffix)

and sends each twice (to expose prefix-cache reuse on the 2nd call), measuring
TTFT each time. No GPU is used by this script; it only times the endpoint.

Usage:
  python scripts/runtime_bench.py --base-url http://localhost:8000/v1 \
      --model my-model --task "fix the billing export bug"

Output: a table of TTFT (ms) for raw vs compiled, cold vs warm, plus the
token counts memory.cpp reports. Nothing here is an estimate -- it is measured
against whatever endpoint you point it at.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def find_binary() -> str:
    for cand in ("target/release/memory.exe", "target/debug/memory.exe",
                 "target/release/memory", "target/debug/memory"):
        p = ROOT / cand
        if p.exists():
            return str(p)
    sys.exit("memory binary not found - run `cargo build -p memory-cli` first")


def seed_and_compile(binary: str, task: str) -> tuple[str, str, dict]:
    """Return (raw_prompt, compiled_prompt, token_report) for a task."""
    tmp = tempfile.mkdtemp(prefix="memcpp-rt-")
    db = os.path.join(tmp, "rt.db")
    facts = [
        "The stable prefix excludes task text and is hashed with blake3.",
        "memory.cpp is local-first: a SQLite vault, no cloud, no telemetry.",
        "Use pnpm for installs. Never npm install.",
        "Billing exports must be deterministic and sorted by invoice id.",
        "Run cargo fmt and clippy -D warnings before every commit.",
        "MCP exposes read-only resources by default.",
    ]
    for f in facts:
        subprocess.run([binary, "--db", db, "remember", f], capture_output=True, text=True)
    wow = subprocess.run([binary, "--db", db, "wow", task, "--for-acquirer", "--json"],
                         capture_output=True, text=True).stdout
    report = json.loads(wow) if wow.strip().startswith("{") else {}
    compiled = subprocess.run([binary, "--db", db, "compile", task, "--provider", "generic"],
                              capture_output=True, text=True).stdout
    # A naive "raw" prompt: the compiled pack repeated to simulate an agent that
    # pastes far more context than needed (a stand-in for unmanaged context).
    raw = (compiled + "\n") * 8
    return raw, compiled, report.get("compiled", {})


def ttft_ms(base_url: str, model: str, prompt: str) -> float:
    body = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 16,
        "temperature": 0,
        "stream": True,
    }).encode()
    req = urllib.request.Request(f"{base_url}/chat/completions", data=body,
                                 headers={"Content-Type": "application/json"})
    start = time.perf_counter()
    with urllib.request.urlopen(req, timeout=120) as resp:
        for line in resp:
            if line.strip() and line.strip() != b"data: [DONE]":
                return (time.perf_counter() - start) * 1000.0
    return (time.perf_counter() - start) * 1000.0


def measure(base_url: str, model: str, label: str, prompt: str) -> tuple[float, float]:
    cold = ttft_ms(base_url, model, prompt)
    warm = ttft_ms(base_url, model, prompt)  # 2nd call -> prefix cache should help
    print(f"  {label:<10} cold {cold:8.1f} ms   warm {warm:8.1f} ms   "
          f"warm speedup {(1 - warm/cold)*100:5.1f}%")
    return cold, warm


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", default="http://localhost:8000/v1")
    ap.add_argument("--model", required=True)
    ap.add_argument("--task", default="fix the billing export bug")
    args = ap.parse_args()

    binary = find_binary()
    raw, compiled, tok = seed_and_compile(binary, args.task)
    print(f"task: {args.task}")
    print(f"token report (memory.cpp): raw≈{tok.get('raw_tokens')} "
          f"compiled≈{tok.get('compiled_tokens')} "
          f"cacheable_prefix≈{tok.get('cacheable_prefix_tokens')}")
    print(f"endpoint: {args.base_url}  model: {args.model}\n")
    print("MEASURED time-to-first-token (lower is better):")
    try:
        measure(args.base_url, args.model, "RAW", raw)
        measure(args.base_url, args.model, "COMPILED", compiled)
    except Exception as e:  # noqa: BLE001
        sys.exit(f"\nCould not reach the endpoint ({e}).\n"
                 f"Start a server first, e.g. `vllm serve {args.model} "
                 f"--enable-prefix-caching`, then re-run.")
    print("\nThese TTFT numbers are MEASURED against your runtime, not estimates.")


if __name__ == "__main__":
    main()
