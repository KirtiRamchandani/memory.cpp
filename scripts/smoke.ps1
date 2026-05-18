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
cargo run -p memory-cli -- --db $db map . --workspace smoke-demo --type evolution --output html --save $mapHtml
cargo run -p memory-cli -- --db $db doctor --workspace smoke-demo
cargo run -p memory-cli -- --db $db start --workspace smoke-demo
Start-Sleep -Seconds 2
cargo run -p memory-cli -- --db $db status
cargo run -p memory-cli -- --db $db stop

$mcpRequest = '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
$mcpResponse = $mcpRequest | cargo run -q -p memory-cli -- --db $db mcp --workspace smoke-demo
if ($mcpResponse -notmatch 'memory_map' -or $mcpResponse -notmatch 'memory_add_candidate') {
    throw 'MCP tools/list did not include the expected safe launch tools.'
}

if (-not (Test-Path $mapHtml)) {
    throw 'Expected evolution.html to be generated during smoke test.'
}

Write-Host 'Smoke test passed.'
