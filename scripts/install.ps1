param(
    [switch]$DryRun,
    [string]$Repo = $env:MEMORY_CPP_REPO,
    [string]$Version = $env:MEMORY_CPP_VERSION,
    [string]$BinDir = $env:MEMORY_CPP_BIN_DIR
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $Repo) { $Repo = 'KirtiRamchandani/memory.cpp' }
if (-not $Version) { $Version = 'latest' }
if (-not $BinDir) { $BinDir = Join-Path $env:USERPROFILE '.memory.cpp\bin' }

$arch = if ([Environment]::Is64BitOperatingSystem) { 'x86_64' } else { 'x86' }
if ($env:PROCESSOR_ARCHITECTURE -match 'ARM64') { $arch = 'aarch64' }
$asset = "memory-windows-$arch.zip"

Write-Host 'memory.cpp installer'
Write-Host "repo: $Repo"
Write-Host "target: windows/$arch"
Write-Host "bin dir: $BinDir"

if ($DryRun) {
    Write-Host "dry run: would try GitHub release asset $asset, verify checksum if present, then fall back to cargo install."
    Write-Host 'next after install: memory setup --developer --yes'
    exit 0
}

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

function Install-FromCargo {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) {
        $env:Path = "$cargoBin;$env:Path"
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'cargo was not found. Install Rust from https://rustup.rs/ or download a release binary.'
    }
    Push-Location (Join-Path $PSScriptRoot '..')
    try {
        $cargoRoot = Split-Path -Parent $BinDir
        cargo install --path crates/memory-cli --force --root $cargoRoot
    } finally {
        Pop-Location
    }
}

function Try-InstallFromRelease {
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("memory-cpp-install-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    try {
        if ($Version -eq 'latest') {
            $url = "https://github.com/$Repo/releases/latest/download/$asset"
            $checksumUrl = "https://github.com/$Repo/releases/latest/download/checksums.txt"
        } else {
            $url = "https://github.com/$Repo/releases/download/$Version/$asset"
            $checksumUrl = "https://github.com/$Repo/releases/download/$Version/checksums.txt"
        }
        Write-Host "trying release asset: $url"
        Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $asset) -UseBasicParsing
        try {
            Invoke-WebRequest -Uri $checksumUrl -OutFile (Join-Path $tmp 'checksums.txt') -UseBasicParsing
            $line = Get-Content (Join-Path $tmp 'checksums.txt') | Where-Object { $_ -match [regex]::Escape($asset) } | Select-Object -First 1
            if ($line) {
                $expected = ($line -split '\s+')[0].ToLowerInvariant()
                $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $asset)).Hash.ToLowerInvariant()
                if ($expected -ne $actual) { throw 'checksum verification failed' }
                Write-Host 'checksum verified'
            }
        } catch {
            Write-Host 'checksum file unavailable; continuing without checksum verification'
        }
        Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath (Join-Path $tmp 'out') -Force
        $binary = Get-ChildItem -Path (Join-Path $tmp 'out') -Recurse -File | Where-Object { $_.Name -match '^memory(\.exe)?$' } | Select-Object -First 1
        if (-not $binary) { return $false }
        Copy-Item $binary.FullName (Join-Path $BinDir 'memory.exe') -Force
        return $true
    } catch {
        Write-Host "release binary unavailable: $($_.Exception.Message)"
        return $false
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

if (-not (Try-InstallFromRelease)) {
    Write-Host 'falling back to cargo install'
    Install-FromCargo
}

Write-Host ''
Write-Host 'Installed memory.cpp.'
Write-Host 'If needed, add this to PATH:'
Write-Host "  $BinDir"
Write-Host ''
Write-Host 'Try:'
Write-Host '  memory welcome'
Write-Host '  memory setup --developer --yes'
Write-Host '  memory doctor'
