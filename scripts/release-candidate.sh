#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo was not found. Install Rust from https://rustup.rs/ and rerun this script." >&2
  exit 1
fi

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run cargo fmt --all -- --check
run cargo check -p memory-cli
run cargo clippy --workspace --all-targets -- -D warnings
run cargo test --workspace
run cargo build -p memory-cli
run git diff --check

if [[ -x scripts/check-docs.sh ]]; then
  run bash scripts/check-docs.sh
fi
if [[ -x scripts/check-website.sh ]]; then
  run bash scripts/check-website.sh
fi

printf '\nRelease-candidate checks passed.\n'
