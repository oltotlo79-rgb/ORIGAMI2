[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $BundleDirectory,
    [Parameter(Mandatory = $true)][string] $PortableExecutable,
    [Parameter(Mandatory = $true)][string] $ExpectedVersion
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$verifier = Join-Path $PSScriptRoot '..\..\scripts\verify_windows_bundle.ps1'
$verifierSource = Get-Content -LiteralPath $verifier -Raw
$identityFunctionStart = $verifierSource.IndexOf('function Assert-BoundedRegularFile')
$identityFunctionEnd = $verifierSource.IndexOf(
    'Assert-BoundedRegularFile -Path $installers', $identityFunctionStart
)
if ($identityFunctionStart -lt 0 -or $identityFunctionEnd -le $identityFunctionStart) {
    throw 'Could not load the Windows file-identity contract.'
}
Invoke-Expression $verifierSource.Substring(
    $identityFunctionStart, $identityFunctionEnd - $identityFunctionStart
)
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
    $resolvedPortable = (Resolve-Path -LiteralPath $PortableExecutable).Path
    $portableDirectory = Split-Path -Parent $resolvedPortable
    $cargoPeerName = ([IO.Path]::GetFileNameWithoutExtension($resolvedPortable) -replace '-', '_') + '.exe'
    $cargoPeer = Join-Path (Join-Path $portableDirectory 'deps') $cargoPeerName
    Assert-BoundedRegularFile -Path $resolvedPortable -Label 'real Cargo portable' `
        -AllowedHardLinkPaths @($cargoPeer)

    $singleDirectory = Join-Path $temporaryRoot 'single-portable'
    New-Item -ItemType Directory -Path $singleDirectory | Out-Null
    $singlePortable = Join-Path $singleDirectory 'app.exe'
    Copy-Item $resolvedPortable $singlePortable
    Assert-BoundedRegularFile -Path $singlePortable -Label 'single portable' `
        -AllowedHardLinkPaths @((Join-Path $singleDirectory 'deps\app.exe'))

    $mismatchedDirectory = Join-Path $temporaryRoot 'mismatched-cargo-peer'
    $mismatchedDeps = Join-Path $mismatchedDirectory 'deps'
    New-Item -ItemType Directory -Path $mismatchedDeps -Force | Out-Null
    $mismatchedPortable = Join-Path $mismatchedDirectory 'app.exe'
    $mismatchedPeer = Join-Path $mismatchedDeps 'app.exe'
    Copy-Item $resolvedPortable $mismatchedPortable
    Copy-Item $resolvedPortable $mismatchedPeer
    try {
        Assert-BoundedRegularFile -Path $mismatchedPortable -Label 'different FileId peer' `
            -AllowedHardLinkPaths @($mismatchedPeer)
        throw 'Validator accepted a same-name Cargo peer with a different FileId.'
    } catch {
        if ($_.Exception.Message -notlike '[[]file-identity[]]*') { throw }
    }

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

    $hostilePortable = Join-Path $temporaryRoot 'hostile-portable.exe'
    Copy-Item $PortableExecutable $hostilePortable
    New-Item -ItemType HardLink -Path (Join-Path $temporaryRoot 'portable-peer.exe') `
        -Target $hostilePortable | Out-Null
    Assert-Rejected unexpected-portable-hardlink $validShape $hostilePortable

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
