Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$required = @(
  'docs/quickstart.md','docs/install.md','docs/uninstall.md','docs/upgrade.md',
  'docs/first-five-minutes.md','docs/core-concepts.md','docs/cli.md','docs/dev-workflow.md',
  'docs/git-memory.md','docs/terminal-memory.md','docs/ai-context.md','docs/context-packs.md',
  'docs/maps.md','docs/inbox.md','docs/doctor.md','docs/privacy.md','docs/safety.md','docs/config.md',
  'docs/ci-memory.md','docs/watch.md','docs/examples.md','docs/troubleshooting.md',
  'docs/troubleshooting-install.md','docs/architecture.md','docs/roadmap.md','docs/faq.md',
  'docs/changelog.md','docs/launch-checklist.md'
)
foreach ($file in $required) { if (-not (Test-Path $file)) { throw "missing $file" } }
Write-Host "Docs check passed ($($required.Count) files)."