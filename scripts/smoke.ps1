Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Run-Memory {
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$MemoryArgs
    )
    & cargo run -p memory-cli -- @MemoryArgs
    if ($LASTEXITCODE -ne 0) {
        throw "memory command failed with exit code ${LASTEXITCODE}: $($MemoryArgs -join ' ')"
    }
}

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) {
    $env:Path = "$cargoBin;$env:Path"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo was not found. Install Rust from https://rustup.rs/ and rerun this script.'
}

$dbDir = Join-Path $PWD '.memory.cpp\smoke'
$db = Join-Path $dbDir 'memory.db'
$mapHtml = Join-Path $dbDir 'evolution.html'
$ciLog = Join-Path $dbDir 'ci.log'
$genericContext = Join-Path $dbDir 'generic-context.md'
$compiledContext = Join-Path $dbDir 'compiled-context.md'
$safeIngest = Join-Path $dbDir 'safe-ingest.md'

if (Test-Path $dbDir) {
    Remove-Item -Recurse -Force $dbDir
}
New-Item -ItemType Directory -Force -Path $dbDir | Out-Null
Set-Content -Path $safeIngest -Value "Smoke ingest note: memory.cpp stores local project facts, commands, decisions, and next steps." -Encoding UTF8

& (Join-Path $PSScriptRoot 'install.ps1') -DryRun
& (Join-Path $PSScriptRoot 'demo-terminal.ps1') -DryRun -Output (Join-Path $dbDir 'terminal-demo')
Run-Memory --db $db init --workspace smoke-demo
Run-Memory --db $db setup --developer --yes --workspace smoke-demo
Run-Memory --db $db what
Run-Memory --db $db where
Run-Memory --db $db today --workspace smoke-demo
Run-Memory --db $db yesterday --workspace smoke-demo
Run-Memory --db $db week --workspace smoke-demo
Run-Memory --db $db next --workspace smoke-demo
Run-Memory --db $db status
Run-Memory --db $db explain memory
Run-Memory --db $db examples dev
Run-Memory --db $db examples list
Run-Memory --db $db examples run billing-export
Run-Memory --db $db privacy status
Run-Memory --db $db demo seed --workspace smoke-demo --path .
Run-Memory --db $db demo multi-model --workspace smoke-demo --path .
Run-Memory --db $db doctor "fix the billing export bug" --provider openai --json
Run-Memory --db $db wow --json "fix checkout bug"
Run-Memory --db $db autopilot "fix checkout bug" --for codex --budget 1500 --output (Join-Path $dbDir 'autopilot-codex.md')
Run-Memory --db $db ship-demo --output (Join-Path $dbDir 'ship-demo.md')
Run-Memory --db $db inbox stats --workspace smoke-demo
Run-Memory --db $db inbox review --workspace smoke-demo
Run-Memory --db $db inbox rules
Run-Memory --db $db inbox rules add "docs/**" --action review
Run-Memory --db $db inbox rules list
Run-Memory --db $db dev morning --workspace smoke-demo
Run-Memory --db $db dev explain-repo . --workspace smoke-demo
Run-Memory --db $db dev next --workspace smoke-demo
Run-Memory --db $db show-context
Run-Memory --db $db context write --for generic --output $genericContext
Run-Memory --db $db context status
Run-Memory --db $db remember "Smoke profile prefers concise summaries." --scope user --type preference
Run-Memory --db $db memories list --limit 5
Run-Memory --db $db profile show --scope user
Run-Memory --db $db profile update "Smoke user prefers local-first reports." --scope user
Run-Memory --db $db mistake "Use cargo fmt before committing Rust changes."
Run-Memory --db $db trace compress --file examples/agent-log.txt
Run-Memory --db $db trace learn --file examples/agent-log.txt
Run-Memory --db $db compile "fix checkout bug" --provider openai --budget 1500 --output $compiledContext
Run-Memory --db $db explain-compile "fix checkout bug" --provider openai
Run-Memory --db $db token-firewall "fix checkout bug" --provider openai --budget 2000
Run-Memory --db $db cache-plan "fix checkout bug" --provider claude
Run-Memory --db $db cache-hash "fix checkout bug"
Run-Memory --db $db cache-stability "fix checkout bug" --provider openai
Run-Memory --db $db kv-report "fix checkout bug"
Run-Memory --db $db prefill-report "fix checkout bug"
Run-Memory --db $db kv-budget "fix checkout bug" --max-kv-tokens 4096
Run-Memory --db $db signal-density "fix checkout bug"
Run-Memory --db $db batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
Run-Memory --db $db runtime-profile list
Run-Memory --db $db cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
Run-Memory --db $db trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
Run-Memory --db $db doctor "add CSV export" --provider gemini
Run-Memory --db $db pack "fix checkout bug" --for generic --budget 1500 --output (Join-Path $dbDir 'generic-pack.md')
Run-Memory --db $db pack "fix checkout bug" --for gemini --budget 1500 --output (Join-Path $dbDir 'gemini-pack.md')
Run-Memory --db $db pack "fix checkout bug" --for mcp --budget 1500 --output (Join-Path $dbDir 'mcp-pack.md')
Run-Memory --db $db explain-pack $genericContext
Run-Memory --db $db context-diff $genericContext $compiledContext
Run-Memory --db $db context-diff latest previous
Run-Memory --db $db blame --pack $genericContext
Run-Memory --db $db ask "what broke last time checkout changed?"
Run-Memory --db $db ask "what broke last time billing changed?"
Run-Memory --db $db suggest "fix checkout bug"
Run-Memory --db $db warnings "change auth flow"
Run-Memory --db $db proactive --task "prepare release"
Run-Memory --db $db trust-report
Run-Memory --db $db redactions
Run-Memory --db $db mcp-scan
Run-Memory --db $db mcp-harden --dry-run --output (Join-Path $dbDir 'mcp-policy.json')
Run-Memory --db $db sign --root $dbDir --output (Join-Path $dbDir 'signatures\manifest.json')
Run-Memory --db $db verify --manifest (Join-Path $dbDir 'signatures\manifest.json')
Run-Memory --db $db quarantine review
Run-Memory --db $db review
Run-Memory --db $db flight start --goal "fix checkout bug" --tool codex
Run-Memory --db $db flight summarize
Run-Memory --db $db flight stop
Run-Memory --db $db test
Run-Memory --db $db ci-check
Run-Memory --db $db ingest file $safeIngest
Run-Memory --db $db shared-context status
Run-Memory --db $db shared-context export --output (Join-Path $dbDir 'shared-context.json')
Run-Memory --db $db heatmap --html --output (Join-Path $dbDir 'heatmap.html')
Run-Memory --db $db report --html --output (Join-Path $dbDir 'memory-report.html')
Run-Memory --db $db dashboard --html --output (Join-Path $dbDir 'dashboard.html')
Run-Memory --db $db agents-score --for codex
Run-Memory --db $db badge --for codex
Run-Memory --db $db recipe list
Run-Memory --db $db recipe apply coding-agent
Run-Memory --db $db preflight --for codex "fix checkout bug"
Run-Memory --db $db roi --input-cost 2.50
Run-Memory --db $db leaderboard
Run-Memory --db $db mistakes
Run-Memory --db $db stale
Run-Memory --db $db conflicts
Run-Memory --db $db savings
Run-Memory --db $db runtime-plan "fix checkout bug" --runtime generic --budget 1200
Run-Memory --db $db runtime-plan "fix checkout bug" --runtime llama.cpp --budget 1200
Run-Memory --db $db bench-context
Run-Memory --db $db config show
Run-Memory --db $db config path
Run-Memory --db $db config profiles
Run-Memory --db $db git summary --since 14d
Run-Memory --db $db git watch --once --dry-run --limit 8
Run-Memory --db $db watch once --dry-run
Run-Memory --db $db watch status
Run-Memory --db $db attach cursor --dry-run
$attachRoot = Join-Path $dbDir 'attach-root'
New-Item -ItemType Directory -Force -Path $attachRoot | Out-Null
Push-Location $attachRoot
try {
    & cargo run --manifest-path (Join-Path $PWD '..\..\..\Cargo.toml') -p memory-cli -- --db $db attach all
    if ($LASTEXITCODE -ne 0) {
        throw "memory attach all failed in smoke sandbox"
    }
} finally {
    Pop-Location
}
Run-Memory --db $db attach --print-config cursor
Run-Memory --db $db attach status
Run-Memory --db $db attach verify cursor --dry-run
Run-Memory --db $db attach export-config cursor
Run-Memory --db $db attach gemini --dry-run
Run-Memory --db $db attach mcp --dry-run
Run-Memory --db $db detach cursor --dry-run
$extractPreview = ((cargo run -q -p memory-cli -- --db $db extract . --workspace smoke-demo --dry-run --limit 5 --json) | Out-String)
if ($extractPreview -notmatch 'candidates') {
    throw 'Expected extract dry-run output to include candidates.'
}
$redactTest = ((cargo run -q -p memory-cli -- --db $db redact test README.md) | Out-String)
if ($redactTest -notmatch 'redaction|No sensitive|no obvious secrets') {
    throw 'Expected redact test to complete.'
}
$redactPreview = ((cargo run -q -p memory-cli -- --db $db redact preview README.md) | Out-String)
if ($redactPreview -notmatch 'README.md|redaction|no obvious secrets') {
    throw 'Expected redact preview to mention the checked path.'
}
$redactionPreview = ((cargo run -q -p memory-cli -- --db $db import . --workspace smoke-demo --preview-redactions --json) | Out-String)
if ($redactionPreview -notmatch 'hits') {
    throw 'Expected import redaction preview output to include hits.'
}
$ignoreCheck = ((cargo run -q -p memory-cli -- --db $db ignore check README.md) | Out-String)
if ($ignoreCheck -notmatch 'included|ignored') {
    throw 'Expected ignore check to report whether the path is included or ignored.'
}
Run-Memory --db $db ignore init --root $dbDir --force
Run-Memory --db $db ignore add smoke-secret.env --root $dbDir
Run-Memory --db $db ignore remove smoke-secret.env --root $dbDir
Run-Memory --db $db map . --workspace smoke-demo --type evolution --output html --save $mapHtml
Run-Memory --db $db show-map --workspace smoke-demo --save (Join-Path $dbDir 'show-map.html')
Run-Memory --db $db map status
Run-Memory --db $db map refresh
Run-Memory --db $db map export-context
Run-Memory --db $db share status --output (Join-Path $dbDir 'project-memory-summary.md')
Run-Memory --db $db share map --output (Join-Path $dbDir 'project-evolution-map.html')
Run-Memory --db $db docs generate --dry-run
Run-Memory --db $db docs list
Run-Memory --db $db docs summarize
Run-Memory --db $db docs search context
Run-Memory --db $db pr summary --base main --output (Join-Path $dbDir 'pr-summary.md')
Run-Memory --db $db pr-comment --base main --output (Join-Path $dbDir 'pr-comment.md')
Run-Memory --db $db pr-context --base main --output (Join-Path $dbDir 'pr-context.md')
Run-Memory --db $db git-learn --since HEAD~1 --dry-run
Run-Memory --db $db branch-summary --base main --output (Join-Path $dbDir 'branch-summary.md')
Run-Memory --db $db timeline week --output (Join-Path $dbDir 'repo-timeline.md')
Run-Memory --db $db rewind last-week --output (Join-Path $dbDir 'rewind.md')
Run-Memory --db $db changed --since "7 days ago" --output (Join-Path $dbDir 'changed.md')
Run-Memory --db $db handoff new-dev --output (Join-Path $dbDir 'handoff')
Run-Memory --db $db adoption status
Run-Memory --db $db release-check
Run-Memory --db $db open --print docs
Run-Memory --db $db doctor --workspace smoke-demo
Run-Memory --db $db fix
Run-Memory --db $db terminal enable --shell powershell
Run-Memory --db $db terminal record --command "cargo test -p memory-cli" --exit-code 0 --duration-ms 1200
Run-Memory --db $db terminal search "how did I run tests?"
Run-Memory --db $db terminal status
Run-Memory --db $db terminal suggest "how did I build release?"
Run-Memory --db $db terminal privacy
@'
Run cargo test
test auth_refresh_retries failed: assertion failed at crates/auth/src/lib.rs:42
error: process did not exit successfully
'@ | Set-Content -Path $ciLog
Run-Memory --db $db ci ingest $ciLog --workspace smoke-demo
Run-Memory --db $db ci explain-failure "auth_refresh_retries" --workspace smoke-demo
Run-Memory --db $db ci report --workspace smoke-demo --output (Join-Path $dbDir 'ci-report.md')
Run-Memory --db $db ci pr-comment --workspace smoke-demo --output (Join-Path $dbDir 'ci-pr-comment.md')
Run-Memory --db $db embeddings explain
Run-Memory --db $db bench --json
Run-Memory --db $db start --workspace smoke-demo
Start-Sleep -Seconds 2
Run-Memory --db $db status
Run-Memory --db $db stop

function Invoke-MemoryMcpJson {
    param([string]$Json, [string]$Name)
    $memoryExe = Join-Path $PWD 'target\debug\memory.exe'
    if (-not (Test-Path $memoryExe)) {
        cargo build -p memory-cli
    }
    $requestPath = Join-Path $dbDir "$Name.jsonl"
    Set-Content -Path $requestPath -Encoding ascii -Value $Json
    $cmd = "`"$memoryExe`" --db `"$db`" mcp --workspace smoke-demo < `"$requestPath`""
    return ((cmd /d /s /c $cmd) | Out-String)
}

$mcpRequest = '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
$mcpResponse = Invoke-MemoryMcpJson -Json $mcpRequest -Name 'mcp-tools-list'
if ($mcpResponse -notmatch 'memory_map' -or $mcpResponse -notmatch 'memory_add_candidate') {
    throw 'MCP tools/list did not include the expected safe launch tools.'
}

$mcpCall = '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_context","arguments":{"query":"MCP integration","workspace":"smoke-demo","tokens":256}}}'
$mcpCallResponse = Invoke-MemoryMcpJson -Json $mcpCall -Name 'mcp-context-call'
if ($mcpCallResponse -notmatch 'MCP integration') {
    throw 'MCP tools/call did not return the expected context payload.'
}

$auditLog = ((cargo run -q -p memory-cli -- --db $db audit-log --limit 5) | Out-String)
if ($auditLog -notmatch 'memory_context') {
    throw 'Expected memory_context access to be visible in the audit log.'
}

if (-not (Test-Path $mapHtml)) {
    throw 'Expected evolution.html to be generated during smoke test.'
}

Write-Host 'Smoke test passed.'
