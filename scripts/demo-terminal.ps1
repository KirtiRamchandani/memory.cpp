param(
    [string]$Output,
    [switch]$Desktop,
    [switch]$DryRun,
    [ValidateSet('none', 'auto', 'asciinema', 'vhs', 'agg')]
    [string]$Record = 'none'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $repoRoot

if ($Desktop) {
    $outDir = Join-Path $HOME 'Desktop\memory.cpp-demo'
} elseif ($Output) {
    $outDir = $Output
} else {
    $outDir = Join-Path $repoRoot '.memory.cpp\reports\demo'
}

New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$dbDir = Join-Path $outDir 'state'
$db = Join-Path $dbDir 'memory.db'
$transcript = Join-Path $outDir 'terminal-demo.txt'
$markdown = Join-Path $outDir 'terminal-demo.md'
$recordingNotes = Join-Path $outDir 'recording-tools.md'
$pack = Join-Path $outDir 'codex-pack.md'
$ship = Join-Path $outDir 'ship-demo.md'

if (Test-Path $dbDir) {
    Remove-Item -Recurse -Force $dbDir
}
New-Item -ItemType Directory -Force -Path $dbDir | Out-Null
Set-Content -Path $transcript -Value '' -Encoding UTF8
Set-Content -Path $markdown -Value "# memory.cpp terminal demo`n`nThis deterministic transcript is safe to paste into a README, issue, launch post, or docs page.`nAll state is local and written under ``.memory.cpp/reports/demo/``.`n" -Encoding UTF8

function Add-TranscriptLine {
    param([string]$Text)
    Add-Content -Path $transcript -Value $Text -Encoding UTF8
}

function Add-MarkdownLine {
    param([string]$Text)
    Add-Content -Path $markdown -Value $Text -Encoding UTF8
}

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) {
    $env:Path = "$cargoBin;$env:Path"
}

function Get-MemoryPrefix {
    if ($env:MEMORY_BIN) {
        return @($env:MEMORY_BIN)
    }
    if (Get-Command memory -ErrorAction SilentlyContinue) {
        return @('memory')
    }
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        return @('cargo', 'run', '-q', '-p', 'memory-cli', '--')
    }
    throw 'cargo was not found and memory is not on PATH. Install Rust from https://rustup.rs/ or set MEMORY_BIN to a built memory binary.'
}

$memoryPrefix = Get-MemoryPrefix

function Invoke-MemoryDemoCommand {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$MemoryArgs)

    $displayPrefix = $memoryPrefix -join ' '
    $display = "$ $displayPrefix --db $db $($MemoryArgs -join ' ')"
    Add-TranscriptLine ''
    Add-TranscriptLine $display
    Add-MarkdownLine ''
    Add-MarkdownLine '```powershell'
    Add-MarkdownLine $display
    Add-MarkdownLine '```'

    if ($DryRun) {
        Add-TranscriptLine '[dry-run] command not executed'
        Add-MarkdownLine ''
        Add-MarkdownLine '```text'
        Add-MarkdownLine '[dry-run] command not executed'
        Add-MarkdownLine '```'
        return
    }

    $cmd = $memoryPrefix[0]
    $prefixArgs = @()
    if ($memoryPrefix.Count -gt 1) {
        $prefixArgs = $memoryPrefix | Select-Object -Skip 1
    }
    $fullArgs = @($prefixArgs) + @('--db', $db) + $MemoryArgs
    $output = & $cmd @fullArgs 2>&1
    $exit = $LASTEXITCODE
    foreach ($line in $output) {
        Add-TranscriptLine ([string]$line)
    }
    Add-MarkdownLine ''
    Add-MarkdownLine '```text'
    foreach ($line in $output) {
        Add-MarkdownLine ([string]$line)
    }
    Add-MarkdownLine '```'
    if ($exit -ne 0) {
        throw "demo command failed with exit code ${exit}: $($MemoryArgs -join ' ')"
    }
}

Add-TranscriptLine 'memory.cpp terminal demo'
Add-TranscriptLine 'Your repo remembers. Remember more. Send less. Run faster.'
Add-TranscriptLine "Artifacts: $outDir"

Invoke-MemoryDemoCommand init --workspace terminal-demo
Invoke-MemoryDemoCommand demo seed --workspace terminal-demo --path .
Invoke-MemoryDemoCommand doctor 'fix the billing export bug' --provider openai
Invoke-MemoryDemoCommand pack 'fix the billing export bug' --for codex --budget 1500 --output $pack
Invoke-MemoryDemoCommand preflight --for codex 'fix the billing export bug'
Invoke-MemoryDemoCommand agents-score
Invoke-MemoryDemoCommand bench --json
Invoke-MemoryDemoCommand ship-demo --output $ship

$recording = @(
    '# Optional recording tools',
    '',
    "Requested recorder: $Record",
    '',
    '| Tool | Detected | Suggested command |',
    '| --- | --- | --- |'
)
foreach ($tool in @('asciinema', 'vhs', 'agg')) {
    if (Get-Command $tool -ErrorAction SilentlyContinue) {
        $recording += "| $tool | yes | Use $tool with the generated terminal-demo.txt transcript or rerun scripts/demo-terminal.sh on Unix-like shells. |"
    } else {
        $recording += "| $tool | no | Install separately if you want this optional recording format. |"
    }
}
Set-Content -Path $recordingNotes -Value $recording -Encoding UTF8

Add-TranscriptLine ''
Add-TranscriptLine 'Terminal demo artifacts written:'
Add-TranscriptLine "- $transcript"
Add-TranscriptLine "- $markdown"
Add-TranscriptLine "- $recordingNotes"
Add-TranscriptLine "- $pack"
Add-TranscriptLine "- $ship"

Write-Host "Terminal demo artifacts written to: $outDir"
