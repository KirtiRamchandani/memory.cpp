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

if (Test-Path $dbDir) {
    Remove-Item -Recurse -Force $dbDir
}
New-Item -ItemType Directory -Force -Path $dbDir | Out-Null

cargo run -p memory-cli -- --db $db init --workspace smoke-demo
cargo run -p memory-cli -- --db $db demo seed --workspace smoke-demo --path .
cargo run -p memory-cli -- --db $db dev morning --workspace smoke-demo
cargo run -p memory-cli -- --db $db dev explain-repo . --workspace smoke-demo
cargo run -p memory-cli -- --db $db dev next --workspace smoke-demo
cargo run -p memory-cli -- --db $db git summary --since 14d
$extractPreview = ((cargo run -q -p memory-cli -- --db $db extract . --workspace smoke-demo --dry-run --limit 5 --json) | Out-String)
if ($extractPreview -notmatch 'candidates') {
    throw 'Expected extract dry-run output to include candidates.'
}
$redactionPreview = ((cargo run -q -p memory-cli -- --db $db import . --workspace smoke-demo --preview-redactions --json) | Out-String)
if ($redactionPreview -notmatch 'hits') {
    throw 'Expected import redaction preview output to include hits.'
}
$ignoreCheck = ((cargo run -q -p memory-cli -- --db $db ignore check README.md) | Out-String)
if ($ignoreCheck -notmatch 'included|ignored') {
    throw 'Expected ignore check to report whether the path is included or ignored.'
}
cargo run -p memory-cli -- --db $db map . --workspace smoke-demo --type evolution --output html --save $mapHtml
cargo run -p memory-cli -- --db $db doctor --workspace smoke-demo
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
