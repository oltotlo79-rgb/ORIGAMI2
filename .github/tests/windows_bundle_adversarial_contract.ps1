[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $BundleDirectory,
    [Parameter(Mandatory = $true)][string] $PortableExecutable,
    [Parameter(Mandatory = $true)][string] $ExpectedVersion
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$verifier = Join-Path $PSScriptRoot '..\..\scripts\verify_windows_bundle.ps1'
$sourceInstaller = @(Get-ChildItem -LiteralPath $BundleDirectory -File -Filter '*.exe')
if ($sourceInstaller.Count -ne 1) { throw 'Expected one source installer fixture.' }
$temporaryRoot = Join-Path $env:RUNNER_TEMP ('origami2-windows-adversarial-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null

function Assert-Rejected {
    param([string] $Label, [string] $FixtureDirectory, [string] $Portable = $PortableExecutable,
        [string] $Version = $ExpectedVersion)
    try {
        & $verifier -BundleDirectory $FixtureDirectory -PortableExecutable $Portable `
            -ExpectedVersion $Version -ExpectedSignatureStatus NotSigned
    } catch {
        return
    }
    throw "Validator accepted adversarial fixture: $Label"
}

try {
    $extra = Join-Path $temporaryRoot 'extra-entry'
    New-Item -ItemType Directory -Path $extra | Out-Null
    Copy-Item $sourceInstaller[0].FullName (Join-Path $extra 'installer.exe')
    Set-Content -LiteralPath (Join-Path $extra 'unexpected.dll') -Value 'unexpected'
    Assert-Rejected extra-dll $extra

    $hardlink = Join-Path $temporaryRoot 'hardlink-installer'
    New-Item -ItemType Directory -Path $hardlink | Out-Null
    $hardlinkInstaller = Join-Path $hardlink 'installer.exe'
    Copy-Item $sourceInstaller[0].FullName $hardlinkInstaller
    New-Item -ItemType HardLink -Path (Join-Path $temporaryRoot 'outside-installer-link.exe') `
        -Target $hardlinkInstaller | Out-Null
    Assert-Rejected hardlink-installer $hardlink

    $reparse = Join-Path $temporaryRoot 'reparse-installer'
    New-Item -ItemType Directory -Path $reparse | Out-Null
    New-Item -ItemType SymbolicLink -Path (Join-Path $reparse 'installer.exe') `
        -Target $sourceInstaller[0].FullName | Out-Null
    Assert-Rejected reparse-installer $reparse

    $oversized = Join-Path $temporaryRoot 'oversized-installer'
    New-Item -ItemType Directory -Path $oversized | Out-Null
    $oversizedInstaller = Join-Path $oversized 'installer.exe'
    Copy-Item $sourceInstaller[0].FullName $oversizedInstaller
    $stream = [IO.File]::OpenWrite($oversizedInstaller)
    try { $stream.SetLength(536870913) } finally { $stream.Dispose() }
    Assert-Rejected oversized-installer $oversized

    $validShape = Join-Path $temporaryRoot 'valid-shape'
    New-Item -ItemType Directory -Path $validShape | Out-Null
    Copy-Item $sourceInstaller[0].FullName (Join-Path $validShape 'installer.exe')
    Assert-Rejected wrong-version $validShape $PortableExecutable '999.0.0'
    Assert-Rejected substituted-portable $validShape $sourceInstaller[0].FullName

    Write-Output 'Windows adversarial bundle contract passed.'
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        $resolved = (Resolve-Path -LiteralPath $temporaryRoot).Path
        $runnerTemp = (Resolve-Path -LiteralPath $env:RUNNER_TEMP).Path.TrimEnd('\')
        if (-not $resolved.StartsWith($runnerTemp + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove unexpected fixture path '$resolved'."
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
