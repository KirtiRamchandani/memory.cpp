#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

OUT_DIR="$REPO_ROOT/.memory.cpp/reports/demo"
DRY_RUN=0
RECORD="none"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      OUT_DIR="$2"
      shift 2
      ;;
    --desktop)
      OUT_DIR="${HOME:-$REPO_ROOT}/Desktop/memory.cpp-demo"
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --record)
      RECORD="$2"
      shift 2
      ;;
    -h|--help)
      cat <<'HELP'
Usage: scripts/demo-terminal.sh [--output DIR] [--desktop] [--dry-run] [--record none|auto|asciinema|vhs|agg]

Creates deterministic terminal-demo artifacts under .memory.cpp/reports/demo by default.
Recording tools are optional. This script never installs VHS, asciinema, or agg.
HELP
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

mkdir -p "$OUT_DIR"
DB_DIR="$OUT_DIR/state"
DB="$DB_DIR/memory.db"
TRANSCRIPT="$OUT_DIR/terminal-demo.txt"
MARKDOWN="$OUT_DIR/terminal-demo.md"
RECORDING_NOTES="$OUT_DIR/recording-tools.md"
PACK="$OUT_DIR/codex-pack.md"
SHIP="$OUT_DIR/ship-demo.md"

rm -rf "$DB_DIR"
mkdir -p "$DB_DIR"
: > "$TRANSCRIPT"
: > "$MARKDOWN"

append() {
  printf '%s\n' "$*" | tee -a "$TRANSCRIPT" >/dev/null
}

append_md() {
  printf '%s\n' "$*" >> "$MARKDOWN"
}

memory_cmd_prefix() {
  if [[ -n "${MEMORY_BIN:-}" ]]; then
    printf '%s\n' "$MEMORY_BIN"
  elif command -v memory >/dev/null 2>&1; then
    printf '%s\n' "memory"
  elif command -v cargo >/dev/null 2>&1; then
    printf '%s\n' "cargo run -q -p memory-cli --"
  else
    echo "cargo was not found and memory is not on PATH." >&2
    echo "Install Rust from https://rustup.rs/ or set MEMORY_BIN to a built memory binary." >&2
    exit 1
  fi
}

PREFIX="$(memory_cmd_prefix)"

run_memory() {
  local args=("$@")
  append ""
  append "$ $PREFIX --db $DB ${args[*]}"
  append_md ""
  append_md '```bash'
  append_md "$ $PREFIX --db $DB ${args[*]}"
  append_md '```'

  if [[ "$DRY_RUN" -eq 1 ]]; then
    append "[dry-run] command not executed"
    append_md ""
    append_md '```text'
    append_md "[dry-run] command not executed"
    append_md '```'
    return 0
  fi

  set +e
  # shellcheck disable=SC2086
  output="$($PREFIX --db "$DB" "${args[@]}" 2>&1)"
  status=$?
  set -e
  printf '%s\n' "$output" | tee -a "$TRANSCRIPT" >/dev/null
  append_md ""
  append_md '```text'
  printf '%s\n' "$output" >> "$MARKDOWN"
  append_md '```'
  if [[ "$status" -ne 0 ]]; then
    echo "demo command failed: ${args[*]}" >&2
    exit "$status"
  fi
}

cat > "$MARKDOWN" <<'MD'
# memory.cpp terminal demo

This deterministic transcript is safe to paste into a README, issue, launch post, or docs page.
All state is local and written under `.memory.cpp/reports/demo/`.
MD

append "memory.cpp terminal demo"
append "Your repo remembers. Remember more. Send less. Run faster."
append "Artifacts: $OUT_DIR"

run_memory init --workspace terminal-demo
run_memory demo seed --workspace terminal-demo --path .
run_memory doctor "fix the billing export bug" --provider openai
run_memory pack "fix the billing export bug" --for codex --budget 1500 --output "$PACK"
run_memory preflight --for codex "fix the billing export bug"
run_memory agents-score
run_memory bench --json
run_memory ship-demo --output "$SHIP"

{
  echo "# Optional recording tools"
  echo
  echo "Requested recorder: $RECORD"
  echo
  echo "| Tool | Detected | Suggested command |"
  echo "| --- | --- | --- |"
  if command -v asciinema >/dev/null 2>&1; then
    echo "| asciinema | yes | asciinema rec \"$OUT_DIR/terminal-demo.cast\" --overwrite --command \"scripts/demo-terminal.sh --output '$OUT_DIR/asciinema-run' --record none\" |"
  else
    echo "| asciinema | no | Install separately if you want a .cast recording. |"
  fi
  if command -v vhs >/dev/null 2>&1; then
    echo "| VHS | yes | vhs \"$OUT_DIR/demo.tape\" |"
  else
    echo "| VHS | no | Install separately if you want a GIF workflow. |"
  fi
  if command -v agg >/dev/null 2>&1; then
    echo "| agg | yes | agg \"$OUT_DIR/terminal-demo.cast\" \"$OUT_DIR/terminal-demo.gif\" |"
  else
    echo "| agg | no | Install separately if you want to render asciinema to GIF. |"
  fi
} > "$RECORDING_NOTES"

append ""
append "Terminal demo artifacts written:"
append "- $TRANSCRIPT"
append "- $MARKDOWN"
append "- $RECORDING_NOTES"
append "- $PACK"
append "- $SHIP"

