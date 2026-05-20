#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo 'cargo was not found. Install Rust from https://rustup.rs/ and rerun this script.' >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

DB_DIR="$REPO_ROOT/.memory.cpp/demo-run"
DB="$DB_DIR/memory.db"
MAP="$DB_DIR/project-map.html"
rm -rf "$DB_DIR"
mkdir -p "$DB_DIR"

run() {
  printf '\n$ %s\n' "$*"
  "$@"
}

echo 'memory.cpp demo'
echo 'Your repo remembers what happened, why it changed, and what to do next.'
echo 'Everything in this demo stays under .memory.cpp/demo-run.'

run cargo run -q -p memory-cli -- --db "$DB" setup --developer --yes --workspace demo
run cargo run -q -p memory-cli -- --db "$DB" demo seed --workspace demo --path .
run cargo run -q -p memory-cli -- --db "$DB" dev morning --workspace demo
run cargo run -q -p memory-cli -- --db "$DB" dev context --for cursor --workspace demo --tokens 900
run cargo run -q -p memory-cli -- --db "$DB" map . --workspace demo --type evolution --output html --save "$MAP"
run cargo run -q -p memory-cli -- --db "$DB" open --print map

echo
echo "Map written to: $MAP"
echo 'Try next: memory dev next --workspace demo'
