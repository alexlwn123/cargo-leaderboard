# Install a checksum-verified GitHub release. Inspect this file before running it.
param(
    [string]$Version = 'latest',
    [string]$BinDir = $(if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME 'bin' } else { Join-Path $env:USERPROFILE '.cargo\bin' })
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if ($Version -ne 'latest' -and $Version -notmatch '^v[0-9][a-zA-Z0-9._-]*$') { throw 'Version must be latest or a v-prefixed release tag.' }
if ([Environment]::OSVersion.Platform -ne 'Win32NT' -or -not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') {
    throw 'This installer supports Windows x64. For other platforms, see the source installation in the README.'
}
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$archive = 'cargo-leaderboard-x86_64-pc-windows-msvc.zip'
$base = 'https://github.com/alexlwn123/cargo-leaderboard/releases'
if ($Version -eq 'latest') { $base += '/latest/download' } else { $base += "/download/$Version" }
$work = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$staged = $null
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "Downloading $Version for Windows x64..."
    Invoke-WebRequest -UseBasicParsing "$base/$archive" -OutFile (Join-Path $work $archive)
    Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS" -OutFile (Join-Path $work 'SHA256SUMS')
    $matches = @(Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object { $_ -match ('^[a-fA-F0-9]{64}\s+' + [regex]::Escape($archive) + '$') })
    if ($matches.Count -ne 1) { throw 'Missing or invalid release checksum.' }
    $expected = ($matches[0] -split '\s+')[0]
    $actual = (Get-FileHash (Join-Path $work $archive) -Algorithm SHA256).Hash
    if ($actual -ne $expected) { throw 'Checksum mismatch. Nothing was installed.' }
    Expand-Archive -LiteralPath (Join-Path $work $archive) -DestinationPath (Join-Path $work 'unpacked')
    $binary = Join-Path $work 'unpacked\cargo-leaderboard.exe'
    & $binary --version
    if ($LASTEXITCODE -ne 0) { throw 'Downloaded binary could not run. Nothing was installed.' }
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    $BinDir = (Resolve-Path -LiteralPath $BinDir).Path
    $destination = Join-Path $BinDir 'cargo-leaderboard.exe'
    $staged = Join-Path $BinDir ('.cargo-leaderboard-' + [guid]::NewGuid().ToString() + '.exe')
    Copy-Item -LiteralPath $binary -Destination $staged
    if (Test-Path -LiteralPath $destination) { [IO.File]::Replace($staged, $destination, $null) }
    else { [IO.File]::Move($staged, $destination) }
    $staged = $null
    Write-Host "Installed to $destination"
    if (($env:PATH -split ';').TrimEnd('\') -notcontains $BinDir.TrimEnd('\')) {
        Write-Host "Add this directory to your user PATH, then reopen your terminal: $BinDir"
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Write-Host 'Install Rust from https://rustup.rs and reopen your terminal.' }
    Write-Host 'Next: cargo leaderboard setup'
    Write-Host 'Update later by running this installer again. Your saved nickname is preserved.'
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    if ($staged) { Remove-Item -LiteralPath $staged -Force -ErrorAction SilentlyContinue }
}
