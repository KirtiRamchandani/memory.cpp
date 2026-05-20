Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) {
    $env:Path = "$cargoBin;$env:Path"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo was not found. Install Rust from https://rustup.rs/ and rerun this script.'
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$dbDir = Join-Path $repoRoot '.memory.cpp\demo-run'
$db = Join-Path $dbDir 'memory.db'
$map = Join-Path $dbDir 'project-map.html'

if (Test-Path $dbDir) {
    Remove-Item -Recurse -Force $dbDir
}
New-Item -ItemType Directory -Force -Path $dbDir | Out-Null
Set-Location $repoRoot

function Invoke-DemoCommand {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Command)
    Write-Host ''
    Write-Host ('$ ' + ($Command -join ' '))
    & $Command[0] @($Command | Select-Object -Skip 1)
}

Write-Host 'memory.cpp demo'
Write-Host 'Your repo remembers what happened, why it changed, and what to do next.'
Write-Host 'Everything in this demo stays under .memory.cpp\demo-run.'

Invoke-DemoCommand cargo run -q -p memory-cli -- --db $db setup --developer --yes --workspace demo
Invoke-DemoCommand cargo run -q -p memory-cli -- --db $db demo seed --workspace demo --path .
Invoke-DemoCommand cargo run -q -p memory-cli -- --db $db dev morning --workspace demo
Invoke-DemoCommand cargo run -q -p memory-cli -- --db $db dev context --for cursor --workspace demo --tokens 900
Invoke-DemoCommand cargo run -q -p memory-cli -- --db $db map . --workspace demo --type evolution --output html --save $map
Invoke-DemoCommand cargo run -q -p memory-cli -- --db $db open --print map

Write-Host ''
Write-Host "Map written to: $map"
Write-Host 'Try next: memory dev next --workspace demo'
