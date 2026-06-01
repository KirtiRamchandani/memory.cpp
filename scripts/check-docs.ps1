Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$required = @(
  'docs/quickstart.md','docs/install.md','docs/uninstall.md','docs/upgrade.md',
  'docs/first-five-minutes.md','docs/core-concepts.md','docs/cli.md','docs/dev-workflow.md',
  'docs/git-memory.md','docs/terminal-memory.md','docs/ai-context.md','docs/context-packs.md',
  'docs/maps.md','docs/inbox.md','docs/doctor.md','docs/privacy.md','docs/safety.md','docs/config.md',
  'docs/ci-memory.md','docs/watch.md','docs/examples.md','docs/troubleshooting.md',
  'docs/troubleshooting-install.md','docs/architecture.md','docs/roadmap.md','docs/faq.md',
  'docs/changelog.md','docs/launch-checklist.md','docs/share.md','docs/pr-workflow.md',
  'docs/timeline.md','docs/handoff.md','docs/adoption.md','docs/context-compiler.md',
  'docs/inference-bottlenecks.md',
  'docs/release-hardening.md','docs/api-stability.md','docs/compatibility.md',
  'docs/limitations.md','docs/performance.md','docs/security.md',
  'docs/dogfooding.md','docs/release-process.md','docs/community.md',
  'docs/providers.md','docs/advanced.md','docs/api.md','docs/demo-script.md',
  'docs/competitive-positioning.md',
  'docs/recipes/optimize-ai-context.md','docs/recipes/avoid-repeat-ai-mistakes.md'
)
foreach ($file in $required) { if (-not (Test-Path $file)) { throw "missing $file" } }
Write-Host "Docs check passed ($($required.Count) files)."
