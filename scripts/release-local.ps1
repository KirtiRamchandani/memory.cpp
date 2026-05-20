Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path dist | Out-Null
cargo build --release -p memory-cli
Copy-Item target/release/memory.exe dist/memory.exe -Force
$hash = (Get-FileHash dist/memory.exe -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  memory.exe" | Set-Content dist/checksums.txt
Write-Host 'Local release artifact: dist/memory.exe'
Write-Host 'Checksum: dist/checksums.txt'