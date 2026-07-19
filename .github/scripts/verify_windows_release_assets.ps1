[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $ReleaseDirectory,

    [Parameter(Mandatory = $true)]
    [string] $Version,

    [Parameter(Mandatory = $true)]
    [string] $CommitSha,

    [switch] $RequireUnsigned
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($Version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw "Release version must be canonical stable SemVer."
}
if ($CommitSha -cnotmatch '^[0-9a-f]{40}$') {
    throw "Release commit must be a full lowercase 40-character Git SHA."
}

$resolvedReleaseDirectory = (Resolve-Path -LiteralPath $ReleaseDirectory).Path
$installerName = "ORIGAMI2-v$Version-windows-x64-unsigned-setup.exe"
$expectedNames = @(
    $installerName,
    'SHA256SUMS.txt',
    'release-notes.md'
) | Sort-Object
$files = @(Get-ChildItem -LiteralPath $resolvedReleaseDirectory -File)
$actualNames = @($files.Name | Sort-Object)
if (($actualNames -join "`n") -cne ($expectedNames -join "`n")) {
    throw (
        "Release directory must contain exactly the installer, SHA256SUMS.txt, " +
        "and release-notes.md."
    )
}
foreach ($file in $files) {
    if (($file.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Release asset '$($file.Name)' must not be a reparse point."
    }
    if ($file.Length -le 0) {
        throw "Release asset '$($file.Name)' must not be empty."
    }
}

$installerPath = Join-Path $resolvedReleaseDirectory $installerName
if ($RequireUnsigned) {
    $signature = Get-AuthenticodeSignature -LiteralPath $installerPath
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
        throw "Release installer must remain explicitly unsigned for the initial release."
    }
}

$checksumPath = Join-Path $resolvedReleaseDirectory 'SHA256SUMS.txt'
$checksumText = [System.IO.File]::ReadAllText($checksumPath)
$escapedInstallerName = [regex]::Escape($installerName)
$checksumMatch = [regex]::Match(
    $checksumText,
    "^([0-9a-f]{64})  ($escapedInstallerName)\n$"
)
if (-not $checksumMatch.Success) {
    throw "SHA256SUMS.txt must contain one canonical lowercase checksum line."
}
$actualDigest = (
    Get-FileHash -LiteralPath $installerPath -Algorithm SHA256
).Hash.ToLowerInvariant()
if ($actualDigest -cne $checksumMatch.Groups[1].Value) {
    throw "Windows installer SHA-256 does not match SHA256SUMS.txt."
}

$releaseNotes = [System.IO.File]::ReadAllText(
    (Join-Path $resolvedReleaseDirectory 'release-notes.md')
)
$releaseNotesTemplate = [System.IO.File]::ReadAllText(
    (Resolve-Path -LiteralPath (
        Join-Path $PSScriptRoot '..\release-notes-windows.md'
    )).Path
)
$releaseNotesContract = [System.IO.File]::ReadAllText(
    (Resolve-Path -LiteralPath (
        Join-Path $PSScriptRoot '..\release-notes-windows-contract.json'
    )).Path
) | ConvertFrom-Json
if ($releaseNotesContract.schema -cne 'origami2.windows-release-notes.v1') {
    throw "Windows release notes contract schema is unsupported."
}
$expectedReleaseNotes = $releaseNotesTemplate.Replace('{{VERSION}}', $Version)
$expectedReleaseNotes = $expectedReleaseNotes.Replace('{{COMMIT_SHA}}', $CommitSha)
$expectedReleaseNotes = $expectedReleaseNotes.Replace(
    '{{INSTALLER_NAME}}',
    $installerName
).Trim() + "`n"
if ($releaseNotes -cne $expectedReleaseNotes) {
    throw "Windows release notes differ from the reviewed bilingual template."
}
foreach ($noticeGroup in @(
    @($releaseNotesContract.requiredJapaneseNotices),
    @($releaseNotesContract.requiredEnglishNotices)
)) {
    if ($noticeGroup.Count -lt 3) {
        throw "Windows release notes must contract at least three notices per language."
    }
    foreach ($notice in $noticeGroup) {
        if ($notice -isnot [string] -or
            [string]::IsNullOrWhiteSpace($notice) -or
            -not $releaseNotes.Contains($notice)) {
            throw "Windows release notes are missing a contracted bilingual notice."
        }
    }
}

Write-Output "Windows release assets verified: $installerName ($actualDigest)."
