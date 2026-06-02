param(
    [string]$Repo,
    [string]$Output,
    [switch]$DryRun,
    [switch]$Keep
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
if (-not $Repo) {
    $Repo = $repoRoot.Path
}
if (-not $Output) {
    $Output = Join-Path $repoRoot '.memory.cpp\fresh-clone-acceptance'
}
if ((-not [System.IO.Path]::IsPathRooted($Repo)) -and ($Repo -notmatch '^[a-zA-Z][a-zA-Z0-9+.-]*://')) {
    $Repo = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $Repo))
}
if (-not [System.IO.Path]::IsPathRooted($Output)) {
    $Output = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $Output))
} else {
    $Output = [System.IO.Path]::GetFullPath($Output)
}

$outDir = $Output
$workRoot = Join-Path $outDir 'work'
$cloneDir = Join-Path $workRoot 'memory.cpp'
$artifactDir = Join-Path $outDir 'artifacts'
$transcript = Join-Path $artifactDir 'acceptance-transcript.txt'
$summary = Join-Path $artifactDir 'summary.md'
$doctorJson = Join-Path $artifactDir 'doctor-openai.json'
$codexPack = Join-Path $artifactDir 'codex-pack.md'
$benchJson = Join-Path $artifactDir 'bench.json'

function Write-Step {
    param([string]$Text)
    Write-Host "[fresh-clone] $Text"
}

function Ensure-Tool {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name was not found on PATH. Install it before running fresh-clone acceptance."
    }
}

$planned = @(
    'git clone <repo> <temp-worktree>',
    'cargo build -p memory-cli',
    'target/debug/memory --db <acceptance-db> init --workspace fresh-clone',
    'target/debug/memory --db <acceptance-db> setup --developer --yes --workspace fresh-clone',
    'target/debug/memory --db <acceptance-db> wow --json "fix billing export bug"',
    'target/debug/memory --db <acceptance-db> demo seed --workspace fresh-clone --path .',
    'target/debug/memory --db <acceptance-db> demo multi-model --workspace fresh-clone --path .',
    'target/debug/memory --db <acceptance-db> doctor "fix the billing export bug" --provider openai --json',
    'target/debug/memory --db <acceptance-db> pack "fix the billing export bug" --for codex --budget 1500 --output <codex-pack>',
    'target/debug/memory --db <acceptance-db> attach all',
    'target/debug/memory --db <acceptance-db> preflight --for codex "fix the billing export bug"',
    'target/debug/memory --db <acceptance-db> agents-score --for codex',
    'target/debug/memory --db <acceptance-db> cache-audit --provider openai --file tests/fixtures/inference/provider_cache_bad_order.md',
    'target/debug/memory --db <acceptance-db> context-diff latest previous',
    'target/debug/memory --db <acceptance-db> ask "what broke last time billing changed?"',
    'target/debug/memory --db <acceptance-db> test',
    'target/debug/memory --db <acceptance-db> bench --json',
    'target/debug/memory --db <acceptance-db> release-check'
)

if ($DryRun) {
    New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
    Set-Content -Path $transcript -Encoding UTF8 -Value (($planned | ForEach-Object { "DRY RUN: $_" }) -join "`n")
    Set-Content -Path $summary -Encoding UTF8 -Value @"
# Fresh clone acceptance dry run

Repo: $Repo
Output: $outDir

The real run clones the repo, builds `memory-cli`, and executes the release acceptance loop from the built `memory` binary instead of `cargo run`.
"@
    Write-Step "dry run complete: $artifactDir"
    exit 0
}

Ensure-Tool git
Ensure-Tool cargo

if (Test-Path $workRoot) {
    Remove-Item -Recurse -Force $workRoot
}
New-Item -ItemType Directory -Force -Path $workRoot | Out-Null
if (Test-Path $artifactDir) {
    Remove-Item -Recurse -Force $artifactDir
}
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
Set-Content -Path $transcript -Encoding UTF8 -Value ''

function Invoke-Logged {
    param(
        [string]$Display,
        [string]$FilePath,
        [string[]]$Arguments,
        [string]$WorkingDirectory,
        [string]$CapturePath
    )

    Add-Content -Path $transcript -Encoding UTF8 -Value "`n$ $Display"
    $previousLocation = Get-Location
    $previousErrorPreference = $ErrorActionPreference
    try {
        Set-Location $WorkingDirectory
        $ErrorActionPreference = 'Continue'
        $output = & $FilePath @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorPreference
        Set-Location $previousLocation
    }
    $stdout = if ($output) { ($output | Out-String).TrimEnd() } else { '' }
    if ($stdout) { Add-Content -Path $transcript -Encoding UTF8 -Value $stdout }
    if ($CapturePath) {
        Set-Content -Path $CapturePath -Encoding UTF8 -Value $stdout
    }
    if ($exitCode -ne 0) {
        throw "command failed ($exitCode): $Display"
    }
}

Write-Step "cloning $Repo"
Invoke-Logged "git clone $Repo $cloneDir" 'git' @('clone', $Repo, $cloneDir) $repoRoot.Path $null

Write-Step 'building memory-cli'
Invoke-Logged 'cargo build -p memory-cli' 'cargo' @('build', '-p', 'memory-cli') $cloneDir $null

$memoryExe = Join-Path $cloneDir 'target\debug\memory.exe'
if (-not (Test-Path $memoryExe)) {
    $memoryExe = Join-Path $cloneDir 'target\debug\memory'
}
if (-not (Test-Path $memoryExe)) {
    throw 'built memory binary was not found under target/debug.'
}

$stateDir = Join-Path $cloneDir '.memory.cpp\acceptance'
$db = Join-Path $stateDir 'memory.db'
New-Item -ItemType Directory -Force -Path $stateDir | Out-Null

function Invoke-MemoryAcceptance {
    param(
        [string[]]$MemoryArgs,
        [string]$CapturePath,
        [string]$WorkingDirectory
    )
    if (-not $WorkingDirectory) { $WorkingDirectory = $cloneDir }
    $display = "memory --db $db $($MemoryArgs -join ' ')"
    Invoke-Logged $display $memoryExe (@('--db', $db) + $MemoryArgs) $WorkingDirectory $CapturePath
}

Invoke-MemoryAcceptance @('init', '--workspace', 'fresh-clone') $null $cloneDir
Invoke-MemoryAcceptance @('setup', '--developer', '--yes', '--workspace', 'fresh-clone') $null $cloneDir
Invoke-MemoryAcceptance @('wow', '--json', 'fix billing export bug') $null $cloneDir
Invoke-MemoryAcceptance @('demo', 'seed', '--workspace', 'fresh-clone', '--path', '.') $null $cloneDir
Invoke-MemoryAcceptance @('demo', 'multi-model', '--workspace', 'fresh-clone', '--path', '.') $null $cloneDir
Invoke-MemoryAcceptance @('doctor', 'fix the billing export bug', '--provider', 'openai', '--json') $doctorJson $cloneDir
Invoke-MemoryAcceptance @('pack', 'fix the billing export bug', '--for', 'codex', '--budget', '1500', '--output', $codexPack) $null $cloneDir
$attachRoot = Join-Path $stateDir 'attach-root'
New-Item -ItemType Directory -Force -Path $attachRoot | Out-Null
Invoke-MemoryAcceptance @('attach', 'all') $null $attachRoot
Invoke-MemoryAcceptance @('preflight', '--for', 'codex', 'fix the billing export bug') $null $cloneDir
Invoke-MemoryAcceptance @('agents-score', '--for', 'codex') $null $cloneDir
Invoke-MemoryAcceptance @('cache-audit', '--provider', 'openai', '--file', 'tests/fixtures/inference/provider_cache_bad_order.md') $null $cloneDir
Invoke-MemoryAcceptance @('context-diff', 'latest', 'previous') $null $cloneDir
Invoke-MemoryAcceptance @('ask', 'what broke last time billing changed?') $null $cloneDir
Invoke-MemoryAcceptance @('test') $null $cloneDir
Invoke-MemoryAcceptance @('bench', '--json') $benchJson $cloneDir
Invoke-MemoryAcceptance @('release-check') $null $cloneDir

Set-Content -Path $summary -Encoding UTF8 -Value @"
# Fresh clone acceptance

Repo: $Repo
Clone: $cloneDir
Binary: $memoryExe
Database: $db

Acceptance result: passed.

Artifacts:

- acceptance-transcript.txt
- doctor-openai.json
- codex-pack.md
- bench.json

This run built `memory-cli` once and executed the release acceptance loop from the built `memory` binary, not from `cargo run`.
"@

if ($Keep) {
    Write-Step "acceptance passed; temporary clone kept at $cloneDir"
} else {
    Write-Step "acceptance passed; temporary clone kept at $cloneDir for inspection"
}
Write-Step "artifacts: $artifactDir"
