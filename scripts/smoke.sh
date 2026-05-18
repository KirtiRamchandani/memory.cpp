#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo 'cargo was not found. Install Rust from https://rustup.rs/ and rerun this script.' >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

DB_DIR="$REPO_ROOT/.memory.cpp/smoke"
DB="$DB_DIR/memory.db"
MAP_HTML="$DB_DIR/evolution.html"
rm -rf "$DB_DIR"
mkdir -p "$DB_DIR"

cargo run -p memory-cli -- --db "$DB" init --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" demo seed --workspace smoke-demo --path .
cargo run -p memory-cli -- --db "$DB" dev morning --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" dev explain-repo . --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" dev next --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" git summary --since 14d
EXTRACT_PREVIEW="$(cargo run -q -p memory-cli -- --db "$DB" extract . --workspace smoke-demo --dry-run --limit 5 --json)"
case "$EXTRACT_PREVIEW" in
  *candidates*) ;;
  *) echo 'Expected extract dry-run output to include candidates.' >&2; exit 1 ;;
esac
REDACTION_PREVIEW="$(cargo run -q -p memory-cli -- --db "$DB" import . --workspace smoke-demo --preview-redactions --json)"
case "$REDACTION_PREVIEW" in
  *hits*) ;;
  *) echo 'Expected import redaction preview output to include hits.' >&2; exit 1 ;;
esac
IGNORE_CHECK="$(cargo run -q -p memory-cli -- --db "$DB" ignore check README.md)"
case "$IGNORE_CHECK" in
  *included*|*ignored*) ;;
  *) echo 'Expected ignore check to report whether the path is included or ignored.' >&2; exit 1 ;;
esac
cargo run -p memory-cli -- --db "$DB" map . --workspace smoke-demo --type evolution --output html --save "$MAP_HTML"
cargo run -p memory-cli -- --db "$DB" doctor --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" start --workspace smoke-demo
sleep 2
cargo run -p memory-cli -- --db "$DB" status
cargo run -p memory-cli -- --db "$DB" stop

MCP_RESPONSE="$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}\n' | cargo run -q -p memory-cli -- --db "$DB" mcp --workspace smoke-demo)"
case "$MCP_RESPONSE" in
  *memory_map*memory_add_candidate*|*memory_add_candidate*memory_map*) ;;
  *) echo 'MCP tools/list did not include the expected safe launch tools.' >&2; exit 1 ;;
esac

MCP_CALL_RESPONSE="$(printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_context","arguments":{"query":"MCP integration","workspace":"smoke-demo","tokens":256}}}\n' | cargo run -q -p memory-cli -- --db "$DB" mcp --workspace smoke-demo)"
case "$MCP_CALL_RESPONSE" in
  *MCP\ integration*) ;;
  *) echo 'MCP tools/call did not return the expected context payload.' >&2; exit 1 ;;
esac

AUDIT_LOG="$(cargo run -q -p memory-cli -- --db "$DB" audit-log --limit 5)"
case "$AUDIT_LOG" in
  *memory_context*) ;;
  *) echo 'Expected memory_context access to be visible in the audit log.' >&2; exit 1 ;;
esac

[[ -f "$MAP_HTML" ]]
printf 'Smoke test passed.\n'
