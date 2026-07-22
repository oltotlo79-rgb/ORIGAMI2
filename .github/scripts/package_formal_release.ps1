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
    node (Join-Path $PSScriptRoot 'create_reproducible_release_archive.mjs') `
        $env:PLATFORM (Join-Path $output "$prefix-portable.zip") $releaseRoot
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the reproducible Windows archive.' }
} elseif ($env:PLATFORM -eq 'macos-arm64') {
    node (Join-Path $PSScriptRoot 'create_reproducible_release_archive.mjs') `
        $env:PLATFORM (Join-Path $output "$prefix-app.tar.gz") $releaseRoot
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the reproducible macOS archive.' }
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
