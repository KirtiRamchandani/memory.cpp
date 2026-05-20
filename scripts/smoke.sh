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
CI_LOG="$DB_DIR/ci.log"
rm -rf "$DB_DIR"
mkdir -p "$DB_DIR"

"$SCRIPT_DIR/install.sh" --dry-run
cargo run -p memory-cli -- --db "$DB" init --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" setup --developer --yes --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" what
cargo run -p memory-cli -- --db "$DB" where
cargo run -p memory-cli -- --db "$DB" today --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" yesterday --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" week --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" next --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" status
cargo run -p memory-cli -- --db "$DB" explain memory
cargo run -p memory-cli -- --db "$DB" examples dev
cargo run -p memory-cli -- --db "$DB" privacy status
cargo run -p memory-cli -- --db "$DB" demo seed --workspace smoke-demo --path .
cargo run -p memory-cli -- --db "$DB" inbox stats --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" inbox review --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" inbox rules
cargo run -p memory-cli -- --db "$DB" inbox rules add "docs/**" --action review
cargo run -p memory-cli -- --db "$DB" inbox rules list
cargo run -p memory-cli -- --db "$DB" dev morning --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" dev explain-repo . --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" dev next --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" show-context
cargo run -p memory-cli -- --db "$DB" context write --for generic --output "$DB_DIR/generic-context.md"
cargo run -p memory-cli -- --db "$DB" context status
cargo run -p memory-cli -- --db "$DB" config show
cargo run -p memory-cli -- --db "$DB" config path
cargo run -p memory-cli -- --db "$DB" config profiles
cargo run -p memory-cli -- --db "$DB" git summary --since 14d
cargo run -p memory-cli -- --db "$DB" git watch --once --dry-run --limit 8
cargo run -p memory-cli -- --db "$DB" watch once --dry-run
cargo run -p memory-cli -- --db "$DB" watch status
cargo run -p memory-cli -- --db "$DB" attach cursor --dry-run
cargo run -p memory-cli -- --db "$DB" attach --print-config cursor
cargo run -p memory-cli -- --db "$DB" attach status
cargo run -p memory-cli -- --db "$DB" detach cursor --dry-run
EXTRACT_PREVIEW="$(cargo run -q -p memory-cli -- --db "$DB" extract . --workspace smoke-demo --dry-run --limit 5 --json)"
case "$EXTRACT_PREVIEW" in
  *candidates*) ;;
  *) echo 'Expected extract dry-run output to include candidates.' >&2; exit 1 ;;
esac
REDACT_TEST="$(cargo run -q -p memory-cli -- --db "$DB" redact test README.md)"
case "$REDACT_TEST" in
  *redaction*|*No\ sensitive*|*'no obvious secrets'*) ;;
  *) echo 'Expected redact test to complete.' >&2; exit 1 ;;
esac
REDACT_PREVIEW="$(cargo run -q -p memory-cli -- --db "$DB" redact preview README.md)"
case "$REDACT_PREVIEW" in
  *README.md*|*redaction*|*'no obvious secrets'*) ;;
  *) echo 'Expected redact preview to mention the checked path.' >&2; exit 1 ;;
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
cargo run -p memory-cli -- --db "$DB" ignore init --root "$DB_DIR" --force
cargo run -p memory-cli -- --db "$DB" ignore add smoke-secret.env --root "$DB_DIR"
cargo run -p memory-cli -- --db "$DB" ignore remove smoke-secret.env --root "$DB_DIR"
cargo run -p memory-cli -- --db "$DB" map . --workspace smoke-demo --type evolution --output html --save "$MAP_HTML"
cargo run -p memory-cli -- --db "$DB" show-map --workspace smoke-demo --save "$DB_DIR/show-map.html"
cargo run -p memory-cli -- --db "$DB" map status
cargo run -p memory-cli -- --db "$DB" map refresh
cargo run -p memory-cli -- --db "$DB" map export-context
cargo run -p memory-cli -- --db "$DB" open --print docs
cargo run -p memory-cli -- --db "$DB" doctor --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" fix
cargo run -p memory-cli -- --db "$DB" terminal enable --shell bash
cargo run -p memory-cli -- --db "$DB" terminal record --command "cargo test -p memory-cli" --exit-code 0 --duration-ms 1200
cargo run -p memory-cli -- --db "$DB" terminal search "how did I run tests?"
cargo run -p memory-cli -- --db "$DB" terminal status
cargo run -p memory-cli -- --db "$DB" terminal suggest "how did I build release?"
cargo run -p memory-cli -- --db "$DB" terminal privacy
cat > "$CI_LOG" <<'LOG'
Run cargo test
test auth_refresh_retries failed: assertion failed at crates/auth/src/lib.rs:42
error: process did not exit successfully
LOG
cargo run -p memory-cli -- --db "$DB" ci ingest "$CI_LOG" --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" ci explain-failure "auth_refresh_retries" --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" ci report --workspace smoke-demo --output "$DB_DIR/ci-report.md"
cargo run -p memory-cli -- --db "$DB" ci pr-comment --workspace smoke-demo --output "$DB_DIR/ci-pr-comment.md"
cargo run -p memory-cli -- --db "$DB" embeddings explain
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
