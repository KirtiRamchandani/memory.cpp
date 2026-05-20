Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
foreach ($file in @('website/index.html','website/styles.css','website/app.js')) { if (-not (Test-Path $file)) { throw "missing $file" } }
$html = Get-Content website/index.html -Raw
$matches = [regex]::Matches($html, 'href="([^"]+)"')
foreach ($match in $matches) {
    $href = $match.Groups[1].Value
    if ($href -match '^(https?:|mailto:|#)' -or [string]::IsNullOrWhiteSpace($href)) { continue }
    $target = Join-Path 'website' $href
    if (-not (Test-Path $target)) { throw "missing website link target: $href" }
}
Write-Host 'Website check passed.'