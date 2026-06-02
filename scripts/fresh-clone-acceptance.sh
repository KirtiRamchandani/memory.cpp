#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="$REPO_ROOT"
OUT_DIR="$REPO_ROOT/.memory.cpp/fresh-clone-acceptance"
DRY_RUN=0
KEEP=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$2"
      shift 2
      ;;
    --output)
      OUT_DIR="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --keep)
      KEEP=1
      shift
      ;;
    -h|--help)
      cat <<'HELP'
Usage: scripts/fresh-clone-acceptance.sh [--repo PATH_OR_URL] [--output DIR] [--dry-run] [--keep]

Clones the repo into a temporary work directory, builds memory-cli once, then runs the
release acceptance loop from the built target/debug/memory binary instead of cargo run.
HELP
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

case "$REPO" in
  *://*|/*) ;;
  *) REPO="$REPO_ROOT/$REPO" ;;
esac

case "$OUT_DIR" in
  /*) ;;
  *) OUT_DIR="$REPO_ROOT/$OUT_DIR" ;;
esac

WORK_ROOT="$OUT_DIR/work"
CLONE_DIR="$WORK_ROOT/memory.cpp"
ARTIFACT_DIR="$OUT_DIR/artifacts"
TRANSCRIPT="$ARTIFACT_DIR/acceptance-transcript.txt"
SUMMARY="$ARTIFACT_DIR/summary.md"
DOCTOR_JSON="$ARTIFACT_DIR/doctor-openai.json"
CODEX_PACK="$ARTIFACT_DIR/codex-pack.md"
BENCH_JSON="$ARTIFACT_DIR/bench.json"

step() {
  printf '[fresh-clone] %s\n' "$*"
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 was not found on PATH. Install it before running fresh-clone acceptance." >&2
    exit 1
  fi
}

planned_commands=(
  'git clone <repo> <temp-worktree>'
  'cargo build -p memory-cli'
  'target/debug/memory --db <acceptance-db> init --workspace fresh-clone'
  'target/debug/memory --db <acceptance-db> setup --developer --yes --workspace fresh-clone'
  'target/debug/memory --db <acceptance-db> wow --json "fix billing export bug"'
  'target/debug/memory --db <acceptance-db> demo --workspace fresh-clone --path .'
  'target/debug/memory --db <acceptance-db> demo seed --workspace fresh-clone --path .'
  'target/debug/memory --db <acceptance-db> demo multi-model --workspace fresh-clone --path .'
  'target/debug/memory --db <acceptance-db> doctor "fix the billing export bug" --provider openai --json'
  'target/debug/memory --db <acceptance-db> pack "fix the billing export bug" --for codex --budget 1500 --output <codex-pack>'
  'target/debug/memory --db <acceptance-db> attach all'
  'target/debug/memory --db <acceptance-db> preflight --for codex "fix the billing export bug"'
  'target/debug/memory --db <acceptance-db> agents-score --for codex'
  'target/debug/memory --db <acceptance-db> cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md'
  'target/debug/memory --db <acceptance-db> context-diff latest previous'
  'target/debug/memory --db <acceptance-db> ask "what broke last time billing changed?"'
  'target/debug/memory --db <acceptance-db> test'
  'target/debug/memory --db <acceptance-db> bench --json'
  'target/debug/memory --db <acceptance-db> release-check'
)

if [[ "$DRY_RUN" -eq 1 ]]; then
  mkdir -p "$ARTIFACT_DIR"
  : > "$TRANSCRIPT"
  for command in "${planned_commands[@]}"; do
    printf 'DRY RUN: %s\n' "$command" >> "$TRANSCRIPT"
  done
  cat > "$SUMMARY" <<EOF
# Fresh clone acceptance dry run

Repo: $REPO
Output: $OUT_DIR

The real run clones the repo, builds memory-cli, and executes the release acceptance loop from the built memory binary instead of cargo run.
EOF
  step "dry run complete: $ARTIFACT_DIR"
  exit 0
fi

require_tool git
require_tool cargo

rm -rf "$WORK_ROOT" "$ARTIFACT_DIR"
mkdir -p "$WORK_ROOT" "$ARTIFACT_DIR"
: > "$TRANSCRIPT"

run_logged() {
  local capture=""
  if [[ "$1" == "--capture" ]]; then
    capture="$2"
    shift 2
  fi
  local display="$1"
  shift
  printf '\n$ %s\n' "$display" >> "$TRANSCRIPT"
  if [[ -n "$capture" ]]; then
    "$@" > >(tee "$capture" >> "$TRANSCRIPT") 2> >(tee -a "$TRANSCRIPT" >&2)
  else
    "$@" >> "$TRANSCRIPT" 2>&1
  fi
}

step "cloning $REPO"
run_logged "git clone $REPO $CLONE_DIR" git clone "$REPO" "$CLONE_DIR"

step 'building memory-cli'
run_logged 'cargo build -p memory-cli' bash -lc "cd '$CLONE_DIR' && cargo build -p memory-cli"

MEMORY="$CLONE_DIR/target/debug/memory"
if [[ ! -x "$MEMORY" && -x "$CLONE_DIR/target/debug/memory.exe" ]]; then
  MEMORY="$CLONE_DIR/target/debug/memory.exe"
fi
if [[ ! -x "$MEMORY" ]]; then
  echo 'built memory binary was not found under target/debug.' >&2
  exit 1
fi

STATE_DIR="$CLONE_DIR/.memory.cpp/acceptance"
DB="$STATE_DIR/memory.db"
mkdir -p "$STATE_DIR"

memory_acceptance() {
  local capture=""
  local workdir="$CLONE_DIR"
  if [[ "$1" == "--capture" ]]; then
    capture="$2"
    shift 2
  fi
  if [[ "$1" == "--workdir" ]]; then
    workdir="$2"
    shift 2
  fi
  local display="memory --db $DB $*"
  if [[ -n "$capture" ]]; then
    run_logged --capture "$capture" "$display" bash -lc "cd '$workdir' && '$MEMORY' --db '$DB' \"\$@\"" memory-args "$@"
  else
    run_logged "$display" bash -lc "cd '$workdir' && '$MEMORY' --db '$DB' \"\$@\"" memory-args "$@"
  fi
}

memory_acceptance init --workspace fresh-clone
memory_acceptance setup --developer --yes --workspace fresh-clone
memory_acceptance wow --json 'fix billing export bug'
memory_acceptance demo --workspace fresh-clone --path .
memory_acceptance demo seed --workspace fresh-clone --path .
memory_acceptance demo multi-model --workspace fresh-clone --path .
memory_acceptance --capture "$DOCTOR_JSON" doctor 'fix the billing export bug' --provider openai --json
memory_acceptance pack 'fix the billing export bug' --for codex --budget 1500 --output "$CODEX_PACK"
ATTACH_ROOT="$STATE_DIR/attach-root"
mkdir -p "$ATTACH_ROOT"
memory_acceptance --workdir "$ATTACH_ROOT" attach all
memory_acceptance preflight --for codex 'fix the billing export bug'
memory_acceptance agents-score --for codex
memory_acceptance cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
memory_acceptance context-diff latest previous
memory_acceptance ask 'what broke last time billing changed?'
memory_acceptance test
memory_acceptance --capture "$BENCH_JSON" bench --json
memory_acceptance release-check

cat > "$SUMMARY" <<EOF
# Fresh clone acceptance

Repo: $REPO
Clone: $CLONE_DIR
Binary: $MEMORY
Database: $DB

Acceptance result: passed.

Artifacts:

- acceptance-transcript.txt
- doctor-openai.json
- codex-pack.md
- bench.json

This run built memory-cli once and executed the release acceptance loop from the built memory binary, not from cargo run.
EOF

if [[ "$KEEP" -eq 1 ]]; then
  step "acceptance passed; temporary clone kept at $CLONE_DIR"
else
  step "acceptance passed; temporary clone kept at $CLONE_DIR for inspection"
fi
step "artifacts: $ARTIFACT_DIR"
