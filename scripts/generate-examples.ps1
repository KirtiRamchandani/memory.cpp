Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Get-ChildItem examples -File | Sort-Object Name | ForEach-Object { $_.FullName }
Write-Host 'Examples are static docs. To refresh live examples, run scripts/demo.ps1 and copy concise output back intentionally.'