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

$mcpRequest = '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
$mcpResponse = (($mcpRequest | cargo run -q -p memory-cli -- --db $db mcp --workspace smoke-demo) | Out-String)
if ($mcpResponse -notmatch 'memory_map' -or $mcpResponse -notmatch 'memory_add_candidate') {
    throw 'MCP tools/list did not include the expected safe launch tools.'
}

$mcpCall = '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_context","arguments":{"query":"MCP integration","workspace":"smoke-demo","tokens":256}}}'
$mcpCallResponse = (($mcpCall | cargo run -q -p memory-cli -- --db $db mcp --workspace smoke-demo) | Out-String)
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
