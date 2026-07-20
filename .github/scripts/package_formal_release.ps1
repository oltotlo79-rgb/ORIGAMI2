Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$output = Join-Path $env:GITHUB_WORKSPACE 'target\formal-release'
New-Item -ItemType Directory -Force $output | Out-Null
$prefix = "ORIGAMI2-v$env:VERSION-$env:PLATFORM"

if ($env:PLATFORM -eq 'windows-x64') {
    $installer = Get-ChildItem 'target\release\bundle\nsis' -Filter '*.exe' -File -Recurse
    if (@($installer).Count -ne 1) { throw 'Expected exactly one NSIS installer.' }
    Copy-Item $installer.FullName (Join-Path $output "$prefix-setup.exe")
    Compress-Archive @(
        'target\release\origami2-desktop.exe',
        'target\release\fonts',
        'target\release\licenses'
    ) (Join-Path $output "$prefix-portable.zip")
} elseif ($env:PLATFORM -eq 'macos-arm64') {
    tar -C 'target/release/bundle/macos' -czf (Join-Path $output "$prefix-app.tar.gz") 'ORIGAMI2.app'
    if ($LASTEXITCODE -ne 0) { throw 'Could not archive the macOS application.' }
} else {
    throw "Unsupported platform '$env:PLATFORM'."
}

node .github/scripts/write_update_manifest.mjs $output
if ($LASTEXITCODE -ne 0) { throw 'Could not generate the update manifest.' }

$assets = Get-ChildItem $output -File |
    Where-Object Name -NotLike 'SHA256SUMS-*' |
    Sort-Object -Property Name
$lines = foreach ($asset in $assets) {
    $digest = (Get-FileHash $asset.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$digest  $($asset.Name)"
}
[IO.File]::WriteAllLines((Join-Path $output "SHA256SUMS-$env:PLATFORM.txt"), $lines)
