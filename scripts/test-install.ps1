param([string]$Archive)
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('leaderboard install ' + [guid]::NewGuid())
New-Item -ItemType Directory $root | Out-Null
try {
    $archivePath = (Resolve-Path $Archive).Path
    $env:CARGO_LEADERBOARD_INSTALL_TEST_MIRROR = Join-Path $root 'mirror'
    New-Item -ItemType Directory $env:CARGO_LEADERBOARD_INSTALL_TEST_MIRROR | Out-Null
    $name = Split-Path $archivePath -Leaf
    Copy-Item $archivePath (Join-Path $env:CARGO_LEADERBOARD_INSTALL_TEST_MIRROR $name)
    $sumPath = Join-Path $env:CARGO_LEADERBOARD_INSTALL_TEST_MIRROR 'SHA256SUMS'
    "$((Get-FileHash $archivePath -Algorithm SHA256).Hash.ToLower())  $name" | Set-Content $sumPath
    function Invoke-WebRequest {
        param([switch]$UseBasicParsing, [Parameter(Position=0)][string]$Uri, [string]$OutFile)
        Copy-Item (Join-Path $env:CARGO_LEADERBOARD_INSTALL_TEST_MIRROR ($Uri.Split('/')[-1])) $OutFile
    }
    $bin = Join-Path $root 'bin with spaces'
    $env:CARGO_LEADERBOARD_CONFIG_DIR = Join-Path $root 'config'
    & ./public/install.ps1 -Version v0.3.0 -BinDir $bin
    $binary = Join-Path $bin 'cargo-leaderboard.exe'
    & $binary setup --nickname installer-test
    if ($LASTEXITCODE -ne 0) { throw 'Setup failed' }
    $config = Get-Content (Join-Path $env:CARGO_LEADERBOARD_CONFIG_DIR 'config.json') -Raw
    & ./public/install.ps1 -Version v0.3.0 -BinDir $bin
    if ((Get-Content (Join-Path $env:CARGO_LEADERBOARD_CONFIG_DIR 'config.json') -Raw) -ne $config) { throw 'Upgrade changed setup' }
    $original = (Get-FileHash $binary).Hash
    "$('0' * 64)  $name" | Set-Content $sumPath
    $rejected = $false
    try { & ./public/install.ps1 -Version v0.3.0 -BinDir $bin } catch { $rejected = $true }
    if (-not $rejected) { throw 'Corrupt checksum accepted' }
    if ((Get-FileHash $binary).Hash -ne $original) { throw 'Failed update changed binary' }
    Write-Host 'Windows installer: fresh install, upgrade, saved setup, checksum rejection, paths with spaces passed.'
} finally {
    Remove-Item $root -Force -Recurse -ErrorAction SilentlyContinue
}
