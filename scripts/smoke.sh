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
GENERIC_CONTEXT="$DB_DIR/generic-context.md"
COMPILED_CONTEXT="$DB_DIR/compiled-context.md"
SAFE_INGEST="$DB_DIR/safe-ingest.md"
rm -rf "$DB_DIR"
mkdir -p "$DB_DIR"
printf '%s\n' 'Smoke ingest note: memory.cpp stores local project facts, commands, decisions, and next steps.' > "$SAFE_INGEST"

"$SCRIPT_DIR/install.sh" --dry-run
"$SCRIPT_DIR/demo-terminal.sh" --dry-run --output "$DB_DIR/terminal-demo"
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
cargo run -p memory-cli -- --db "$DB" examples list
cargo run -p memory-cli -- --db "$DB" examples run billing-export
cargo run -p memory-cli -- --db "$DB" privacy status
cargo run -p memory-cli -- --db "$DB" demo seed --workspace smoke-demo --path .
cargo run -p memory-cli -- --db "$DB" demo multi-model --workspace smoke-demo --path .
cargo run -p memory-cli -- --db "$DB" doctor "fix the billing export bug" --provider openai --json
cargo run -p memory-cli -- --db "$DB" wow --json "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" autopilot "fix checkout bug" --for codex --budget 1500 --output "$DB_DIR/autopilot-codex.md"
cargo run -p memory-cli -- --db "$DB" ship-demo --output "$DB_DIR/ship-demo.md"
cargo run -p memory-cli -- --db "$DB" inbox stats --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" inbox review --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" inbox rules
cargo run -p memory-cli -- --db "$DB" inbox rules add "docs/**" --action review
cargo run -p memory-cli -- --db "$DB" inbox rules list
cargo run -p memory-cli -- --db "$DB" dev morning --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" dev explain-repo . --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" dev next --workspace smoke-demo
cargo run -p memory-cli -- --db "$DB" show-context
cargo run -p memory-cli -- --db "$DB" context write --for generic --output "$GENERIC_CONTEXT"
cargo run -p memory-cli -- --db "$DB" context status
cargo run -p memory-cli -- --db "$DB" remember "Smoke profile prefers concise summaries." --scope user --type preference
cargo run -p memory-cli -- --db "$DB" memories list --limit 5
cargo run -p memory-cli -- --db "$DB" profile show --scope user
cargo run -p memory-cli -- --db "$DB" profile update "Smoke user prefers local-first reports." --scope user
cargo run -p memory-cli -- --db "$DB" mistake "Use cargo fmt before committing Rust changes."
cargo run -p memory-cli -- --db "$DB" trace compress --file examples/agent-log.txt
cargo run -p memory-cli -- --db "$DB" trace learn --file examples/agent-log.txt
cargo run -p memory-cli -- --db "$DB" compile "fix checkout bug" --provider openai --budget 1500 --output "$COMPILED_CONTEXT"
cargo run -p memory-cli -- --db "$DB" explain-compile "fix checkout bug" --provider openai
cargo run -p memory-cli -- --db "$DB" token-firewall "fix checkout bug" --provider openai --budget 2000
cargo run -p memory-cli -- --db "$DB" cache-plan "fix checkout bug" --provider claude
cargo run -p memory-cli -- --db "$DB" cache-hash "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" cache-stability "fix checkout bug" --provider openai
cargo run -p memory-cli -- --db "$DB" kv-report "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" prefill-report "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" kv-budget "fix checkout bug" --max-kv-tokens 4096
cargo run -p memory-cli -- --db "$DB" signal-density "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
cargo run -p memory-cli -- --db "$DB" runtime-profile list
cargo run -p memory-cli -- --db "$DB" cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
cargo run -p memory-cli -- --db "$DB" trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
cargo run -p memory-cli -- --db "$DB" doctor "add CSV export" --provider gemini
cargo run -p memory-cli -- --db "$DB" pack "fix checkout bug" --for generic --budget 1500 --output "$DB_DIR/generic-pack.md"
cargo run -p memory-cli -- --db "$DB" pack "fix checkout bug" --for gemini --budget 1500 --output "$DB_DIR/gemini-pack.md"
cargo run -p memory-cli -- --db "$DB" pack "fix checkout bug" --for mcp --budget 1500 --output "$DB_DIR/mcp-pack.md"
cargo run -p memory-cli -- --db "$DB" explain-pack "$GENERIC_CONTEXT"
cargo run -p memory-cli -- --db "$DB" context-diff "$GENERIC_CONTEXT" "$COMPILED_CONTEXT"
cargo run -p memory-cli -- --db "$DB" context-diff latest previous
cargo run -p memory-cli -- --db "$DB" blame --pack "$GENERIC_CONTEXT"
cargo run -p memory-cli -- --db "$DB" ask "what broke last time checkout changed?"
cargo run -p memory-cli -- --db "$DB" ask "what broke last time billing changed?"
cargo run -p memory-cli -- --db "$DB" suggest "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" warnings "change auth flow"
cargo run -p memory-cli -- --db "$DB" proactive --task "prepare release"
cargo run -p memory-cli -- --db "$DB" trust-report
cargo run -p memory-cli -- --db "$DB" redactions
cargo run -p memory-cli -- --db "$DB" mcp-scan
cargo run -p memory-cli -- --db "$DB" mcp-harden --dry-run --output "$DB_DIR/mcp-policy.json"
cargo run -p memory-cli -- --db "$DB" sign --root "$DB_DIR" --output "$DB_DIR/signatures/manifest.json"
cargo run -p memory-cli -- --db "$DB" verify --manifest "$DB_DIR/signatures/manifest.json"
cargo run -p memory-cli -- --db "$DB" quarantine review
cargo run -p memory-cli -- --db "$DB" review
cargo run -p memory-cli -- --db "$DB" flight start --goal "fix checkout bug" --tool codex
cargo run -p memory-cli -- --db "$DB" flight summarize
cargo run -p memory-cli -- --db "$DB" flight stop
cargo run -p memory-cli -- --db "$DB" test
cargo run -p memory-cli -- --db "$DB" ci-check
cargo run -p memory-cli -- --db "$DB" ingest file "$SAFE_INGEST"
cargo run -p memory-cli -- --db "$DB" shared-context status
cargo run -p memory-cli -- --db "$DB" shared-context export --output "$DB_DIR/shared-context.json"
cargo run -p memory-cli -- --db "$DB" heatmap --html --output "$DB_DIR/heatmap.html"
cargo run -p memory-cli -- --db "$DB" report --html --output "$DB_DIR/memory-report.html"
cargo run -p memory-cli -- --db "$DB" dashboard --html --output "$DB_DIR/dashboard.html"
cargo run -p memory-cli -- --db "$DB" agents-score --for codex
cargo run -p memory-cli -- --db "$DB" badge --for codex
cargo run -p memory-cli -- --db "$DB" recipe list
cargo run -p memory-cli -- --db "$DB" recipe apply coding-agent
cargo run -p memory-cli -- --db "$DB" preflight --for codex "fix checkout bug"
cargo run -p memory-cli -- --db "$DB" roi --input-cost 2.50
cargo run -p memory-cli -- --db "$DB" leaderboard
cargo run -p memory-cli -- --db "$DB" mistakes
cargo run -p memory-cli -- --db "$DB" stale
cargo run -p memory-cli -- --db "$DB" conflicts
cargo run -p memory-cli -- --db "$DB" savings
cargo run -p memory-cli -- --db "$DB" runtime-plan "fix checkout bug" --runtime generic --budget 1200
cargo run -p memory-cli -- --db "$DB" runtime-plan "fix checkout bug" --runtime llama.cpp --budget 1200
cargo run -p memory-cli -- --db "$DB" bench-context
cargo run -p memory-cli -- --db "$DB" config show
cargo run -p memory-cli -- --db "$DB" config path
cargo run -p memory-cli -- --db "$DB" config profiles
cargo run -p memory-cli -- --db "$DB" git summary --since 14d
cargo run -p memory-cli -- --db "$DB" git watch --once --dry-run --limit 8
cargo run -p memory-cli -- --db "$DB" watch once --dry-run
cargo run -p memory-cli -- --db "$DB" watch status
cargo run -p memory-cli -- --db "$DB" attach cursor --dry-run
ATTACH_ROOT="$DB_DIR/attach-root"
mkdir -p "$ATTACH_ROOT"
(cd "$ATTACH_ROOT" && cargo run --manifest-path "$REPO_ROOT/Cargo.toml" -p memory-cli -- --db "$DB" attach all)
cargo run -p memory-cli -- --db "$DB" attach --print-config cursor
cargo run -p memory-cli -- --db "$DB" attach status
cargo run -p memory-cli -- --db "$DB" attach verify cursor --dry-run
cargo run -p memory-cli -- --db "$DB" attach export-config cursor
cargo run -p memory-cli -- --db "$DB" attach gemini --dry-run
cargo run -p memory-cli -- --db "$DB" attach mcp --dry-run
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
cargo run -p memory-cli -- --db "$DB" share status --output "$DB_DIR/project-memory-summary.md"
cargo run -p memory-cli -- --db "$DB" share map --output "$DB_DIR/project-evolution-map.html"
cargo run -p memory-cli -- --db "$DB" docs generate --dry-run
cargo run -p memory-cli -- --db "$DB" docs list
cargo run -p memory-cli -- --db "$DB" docs summarize
cargo run -p memory-cli -- --db "$DB" docs search context
cargo run -p memory-cli -- --db "$DB" pr summary --base main --output "$DB_DIR/pr-summary.md"
cargo run -p memory-cli -- --db "$DB" pr-comment --base main --output "$DB_DIR/pr-comment.md"
cargo run -p memory-cli -- --db "$DB" pr-context --base main --output "$DB_DIR/pr-context.md"
cargo run -p memory-cli -- --db "$DB" git-learn --since HEAD~1 --dry-run
cargo run -p memory-cli -- --db "$DB" branch-summary --base main --output "$DB_DIR/branch-summary.md"
cargo run -p memory-cli -- --db "$DB" timeline week --output "$DB_DIR/repo-timeline.md"
cargo run -p memory-cli -- --db "$DB" rewind last-week --output "$DB_DIR/rewind.md"
cargo run -p memory-cli -- --db "$DB" changed --since "7 days ago" --output "$DB_DIR/changed.md"
cargo run -p memory-cli -- --db "$DB" handoff new-dev --output "$DB_DIR/handoff"
cargo run -p memory-cli -- --db "$DB" adoption status
cargo run -p memory-cli -- --db "$DB" release-check
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
cargo run -p memory-cli -- --db "$DB" bench --json
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
