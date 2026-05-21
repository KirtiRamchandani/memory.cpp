Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) {
    $env:Path = "$cargoBin;$env:Path"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found. Install Rust from https://rustup.rs/ and rerun this script."
}

function Invoke-Step {
    param([Parameter(Mandatory=$true)][string]$Name, [Parameter(Mandatory=$true)][scriptblock]$Command)
    Write-Host "`n==> $Name"
    & $Command
}

Invoke-Step "cargo fmt" { cargo fmt --all -- --check }
Invoke-Step "cargo clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Step "cargo test" { cargo test --workspace }
Invoke-Step "cargo build" { cargo build -p memory-cli }
Invoke-Step "git diff check" { git diff --check }
Invoke-Step "docs check" { powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-docs.ps1 }
Invoke-Step "website check" { powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-website.ps1 }

Write-Host "`nRelease-candidate checks passed."