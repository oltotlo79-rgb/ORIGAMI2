Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:VERSION -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw 'Release version must be canonical stable SemVer.'
}
if ($env:PLATFORM -cnotin @('windows-x64', 'macos-arm64')) {
    throw 'Release platform is unsupported.'
}

$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$releaseRoot = Join-Path $repositoryRoot 'target\release'
$output = Join-Path $repositoryRoot 'target\formal-release'
New-Item -ItemType Directory -Force $output | Out-Null
$prefix = "ORIGAMI2-v$env:VERSION-$env:PLATFORM"

if ($env:PLATFORM -eq 'windows-x64') {
    $installer = Get-ChildItem (Join-Path $releaseRoot 'bundle\nsis') -Filter '*.exe' -File -Recurse
    if (@($installer).Count -ne 1) { throw 'Expected exactly one NSIS installer.' }
    Copy-Item $installer.FullName (Join-Path $output "$prefix-setup.exe")
    Compress-Archive @(
        (Join-Path $releaseRoot 'origami2-desktop.exe'),
        (Join-Path $releaseRoot 'fonts'),
        (Join-Path $releaseRoot 'licenses')
    ) (Join-Path $output "$prefix-portable.zip")
} elseif ($env:PLATFORM -eq 'macos-arm64') {
    tar -C (Join-Path $releaseRoot 'bundle/macos') -czf (Join-Path $output "$prefix-app.tar.gz") 'ORIGAMI2.app'
    if ($LASTEXITCODE -ne 0) { throw 'Could not archive the macOS application.' }
}

node (Join-Path $PSScriptRoot 'write_update_manifest.mjs') $output
if ($LASTEXITCODE -ne 0) { throw 'Could not generate the update manifest.' }

$assets = Get-ChildItem $output -File |
    Where-Object Name -NotLike 'SHA256SUMS-*' |
    Sort-Object -Property Name
$lines = foreach ($asset in $assets) {
    $digest = (Get-FileHash $asset.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$digest  $($asset.Name)"
}
[IO.File]::WriteAllLines((Join-Path $output "SHA256SUMS-$env:PLATFORM.txt"), $lines)
