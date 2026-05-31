Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

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

if (Test-Path $dbDir) {
    Remove-Item -Recurse -Force $dbDir
}
New-Item -ItemType Directory -Force -Path $dbDir | Out-Null

& (Join-Path $PSScriptRoot 'install.ps1') -DryRun
cargo run -p memory-cli -- --db $db init --workspace smoke-demo
cargo run -p memory-cli -- --db $db setup --developer --yes --workspace smoke-demo
cargo run -p memory-cli -- --db $db what
cargo run -p memory-cli -- --db $db where
cargo run -p memory-cli -- --db $db today --workspace smoke-demo
cargo run -p memory-cli -- --db $db yesterday --workspace smoke-demo
cargo run -p memory-cli -- --db $db week --workspace smoke-demo
cargo run -p memory-cli -- --db $db next --workspace smoke-demo
cargo run -p memory-cli -- --db $db status
cargo run -p memory-cli -- --db $db explain memory
cargo run -p memory-cli -- --db $db examples dev
cargo run -p memory-cli -- --db $db privacy status
cargo run -p memory-cli -- --db $db demo seed --workspace smoke-demo --path .
cargo run -p memory-cli -- --db $db inbox stats --workspace smoke-demo
cargo run -p memory-cli -- --db $db inbox review --workspace smoke-demo
cargo run -p memory-cli -- --db $db inbox rules
cargo run -p memory-cli -- --db $db inbox rules add "docs/**" --action review
cargo run -p memory-cli -- --db $db inbox rules list
cargo run -p memory-cli -- --db $db dev morning --workspace smoke-demo
cargo run -p memory-cli -- --db $db dev explain-repo . --workspace smoke-demo
cargo run -p memory-cli -- --db $db dev next --workspace smoke-demo
cargo run -p memory-cli -- --db $db show-context
cargo run -p memory-cli -- --db $db context write --for generic --output (Join-Path $dbDir 'generic-context.md')
cargo run -p memory-cli -- --db $db context status
cargo run -p memory-cli -- --db $db mistake "Use cargo fmt before committing Rust changes."
cargo run -p memory-cli -- --db $db trace compress --file examples/agent-log.txt
cargo run -p memory-cli -- --db $db trace learn --file examples/agent-log.txt
cargo run -p memory-cli -- --db $db compile "fix checkout bug" --provider openai --budget 1500 --output (Join-Path $dbDir 'compiled-context.md')
cargo run -p memory-cli -- --db $db token-firewall "fix checkout bug" --provider openai --budget 2000
cargo run -p memory-cli -- --db $db cache-plan "fix checkout bug" --provider claude
cargo run -p memory-cli -- --db $db kv-report "fix checkout bug"
cargo run -p memory-cli -- --db $db prefill-report "fix checkout bug"
cargo run -p memory-cli -- --db $db kv-budget "fix checkout bug" --max-kv-tokens 4096
cargo run -p memory-cli -- --db $db signal-density "fix checkout bug"
cargo run -p memory-cli -- --db $db batch-plan --file tests/fixtures/inference/multi_request_batch.json --provider openai
cargo run -p memory-cli -- --db $db runtime-profile list
cargo run -p memory-cli -- --db $db cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md
cargo run -p memory-cli -- --db $db trace-rollup --from tests/fixtures/inference/agent_trace_long.json --every 50
cargo run -p memory-cli -- --db $db doctor "add CSV export" --provider gemini
cargo run -p memory-cli -- --db $db pack "fix checkout bug" --for generic --budget 1500 --output (Join-Path $dbDir 'generic-pack.md')
cargo run -p memory-cli -- --db $db mistakes
cargo run -p memory-cli -- --db $db stale
cargo run -p memory-cli -- --db $db conflicts
cargo run -p memory-cli -- --db $db savings
cargo run -p memory-cli -- --db $db runtime-plan "fix checkout bug" --runtime generic --budget 1200
cargo run -p memory-cli -- --db $db runtime-plan "fix checkout bug" --runtime llama.cpp --budget 1200
cargo run -p memory-cli -- --db $db bench-context
cargo run -p memory-cli -- --db $db config show
cargo run -p memory-cli -- --db $db config path
cargo run -p memory-cli -- --db $db config profiles
cargo run -p memory-cli -- --db $db git summary --since 14d
cargo run -p memory-cli -- --db $db git watch --once --dry-run --limit 8
cargo run -p memory-cli -- --db $db watch once --dry-run
cargo run -p memory-cli -- --db $db watch status
cargo run -p memory-cli -- --db $db attach cursor --dry-run
cargo run -p memory-cli -- --db $db attach --print-config cursor
cargo run -p memory-cli -- --db $db attach status
cargo run -p memory-cli -- --db $db attach verify cursor --dry-run
cargo run -p memory-cli -- --db $db attach export-config cursor
cargo run -p memory-cli -- --db $db detach cursor --dry-run
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
cargo run -p memory-cli -- --db $db ignore init --root $dbDir --force
cargo run -p memory-cli -- --db $db ignore add smoke-secret.env --root $dbDir
cargo run -p memory-cli -- --db $db ignore remove smoke-secret.env --root $dbDir
cargo run -p memory-cli -- --db $db map . --workspace smoke-demo --type evolution --output html --save $mapHtml
cargo run -p memory-cli -- --db $db show-map --workspace smoke-demo --save (Join-Path $dbDir 'show-map.html')
cargo run -p memory-cli -- --db $db map status
cargo run -p memory-cli -- --db $db map refresh
cargo run -p memory-cli -- --db $db map export-context
cargo run -p memory-cli -- --db $db share status --output (Join-Path $dbDir 'project-memory-summary.md')
cargo run -p memory-cli -- --db $db share map --output (Join-Path $dbDir 'project-evolution-map.html')
cargo run -p memory-cli -- --db $db docs generate --dry-run
cargo run -p memory-cli -- --db $db pr summary --base main --output (Join-Path $dbDir 'pr-summary.md')
cargo run -p memory-cli -- --db $db timeline week --output (Join-Path $dbDir 'repo-timeline.md')
cargo run -p memory-cli -- --db $db rewind last-week --output (Join-Path $dbDir 'rewind.md')
cargo run -p memory-cli -- --db $db changed --since "7 days ago" --output (Join-Path $dbDir 'changed.md')
cargo run -p memory-cli -- --db $db handoff new-dev --output (Join-Path $dbDir 'handoff')
cargo run -p memory-cli -- --db $db adoption status
cargo run -p memory-cli -- --db $db release-check
cargo run -p memory-cli -- --db $db open --print docs
cargo run -p memory-cli -- --db $db doctor --workspace smoke-demo
cargo run -p memory-cli -- --db $db fix
cargo run -p memory-cli -- --db $db terminal enable --shell powershell
cargo run -p memory-cli -- --db $db terminal record --command "cargo test -p memory-cli" --exit-code 0 --duration-ms 1200
cargo run -p memory-cli -- --db $db terminal search "how did I run tests?"
cargo run -p memory-cli -- --db $db terminal status
cargo run -p memory-cli -- --db $db terminal suggest "how did I build release?"
cargo run -p memory-cli -- --db $db terminal privacy
@'
Run cargo test
test auth_refresh_retries failed: assertion failed at crates/auth/src/lib.rs:42
error: process did not exit successfully
'@ | Set-Content -Path $ciLog
cargo run -p memory-cli -- --db $db ci ingest $ciLog --workspace smoke-demo
cargo run -p memory-cli -- --db $db ci explain-failure "auth_refresh_retries" --workspace smoke-demo
cargo run -p memory-cli -- --db $db ci report --workspace smoke-demo --output (Join-Path $dbDir 'ci-report.md')
cargo run -p memory-cli -- --db $db ci pr-comment --workspace smoke-demo --output (Join-Path $dbDir 'ci-pr-comment.md')
cargo run -p memory-cli -- --db $db embeddings explain
cargo run -p memory-cli -- --db $db start --workspace smoke-demo
Start-Sleep -Seconds 2
cargo run -p memory-cli -- --db $db status
cargo run -p memory-cli -- --db $db stop

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
