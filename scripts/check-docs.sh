#!/usr/bin/env bash
set -euo pipefail
required=(
  docs/quickstart.md docs/install.md docs/uninstall.md docs/upgrade.md
  docs/first-five-minutes.md docs/core-concepts.md docs/cli.md docs/dev-workflow.md
  docs/git-memory.md docs/terminal-memory.md docs/ai-context.md docs/context-packs.md
  docs/maps.md docs/inbox.md docs/doctor.md docs/privacy.md docs/safety.md docs/config.md
  docs/ci-memory.md docs/watch.md docs/examples.md docs/troubleshooting.md
  docs/troubleshooting-install.md docs/architecture.md docs/roadmap.md docs/faq.md
  docs/changelog.md docs/launch-checklist.md docs/share.md docs/pr-workflow.md
  docs/timeline.md docs/handoff.md docs/adoption.md
)
for file in "${required[@]}"; do
  [[ -f "$file" ]] || { echo "missing $file" >&2; exit 1; }
done
printf 'Docs check passed (%s files).\n' "${#required[@]}"
