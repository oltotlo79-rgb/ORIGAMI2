[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $BundleDirectory,

    [Parameter(Mandatory = $true)]
    [string] $OutputDirectory,

    [Parameter(Mandatory = $true)]
    [string] $Version,

    [Parameter(Mandatory = $true)]
    [string] $CommitSha,

    [Parameter(Mandatory = $true)]
    [string] $OutputFile
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($Version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw "Release version must be canonical stable SemVer."
}
if ($CommitSha -cnotmatch '^[0-9a-f]{40}$') {
    throw "Release commit must be a full lowercase 40-character Git SHA."
}

$resolvedBundleDirectory = (Resolve-Path -LiteralPath $BundleDirectory).Path
$installers = @(Get-ChildItem -LiteralPath $resolvedBundleDirectory -File -Filter '*.exe')
if ($installers.Count -ne 1) {
    throw "Expected exactly one Windows NSIS installer, found $($installers.Count)."
}
if ($installers[0].Length -le 0) {
    throw "Windows NSIS installer must not be empty."
}

$signature = Get-AuthenticodeSignature -LiteralPath $installers[0].FullName
if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
    throw (
        "Initial release contract requires an explicitly unsigned installer; " +
        "Authenticode status was '$($signature.Status)'."
    )
}

$absoluteOutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $absoluteOutputDirectory) {
    $existing = @(Get-ChildItem -LiteralPath $absoluteOutputDirectory -Force)
    if ($existing.Count -ne 0) {
        throw "Release output directory must be absent or empty."
    }
} else {
    New-Item -ItemType Directory -Path $absoluteOutputDirectory | Out-Null
}

$installerName = "ORIGAMI2-v$Version-windows-x64-unsigned-setup.exe"
$packagedInstaller = Join-Path $absoluteOutputDirectory $installerName
Copy-Item -LiteralPath $installers[0].FullName -Destination $packagedInstaller

$digest = (Get-FileHash -LiteralPath $packagedInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
$checksumPath = Join-Path $absoluteOutputDirectory 'SHA256SUMS.txt'
[System.IO.File]::WriteAllText(
    $checksumPath,
    "$digest  $installerName`n",
    [System.Text.UTF8Encoding]::new($false)
)

$releaseNotesTemplatePath = Join-Path $PSScriptRoot '..\release-notes-windows.md'
$releaseNotesContractPath = Join-Path (
    $PSScriptRoot
) '..\release-notes-windows-contract.json'
$releaseNotesTemplate = [System.IO.File]::ReadAllText(
    (Resolve-Path -LiteralPath $releaseNotesTemplatePath).Path
)
$releaseNotesContract = [System.IO.File]::ReadAllText(
    (Resolve-Path -LiteralPath $releaseNotesContractPath).Path
) | ConvertFrom-Json
if ($releaseNotesContract.schema -cne 'origami2.windows-release-notes.v1') {
    throw "Windows release notes contract schema is unsupported."
}
$noticeGroups = @(
    @($releaseNotesContract.requiredJapaneseNotices),
    @($releaseNotesContract.requiredEnglishNotices)
)
foreach ($noticeGroup in $noticeGroups) {
    if ($noticeGroup.Count -lt 3) {
        throw "Windows release notes contract must require both languages."
    }
    foreach ($notice in $noticeGroup) {
        if ($notice -isnot [string] -or
            [string]::IsNullOrWhiteSpace($notice) -or
            -not $releaseNotesTemplate.Contains($notice)) {
            throw "Windows release notes template is missing a contracted notice."
        }
    }
}
$releaseNotes = $releaseNotesTemplate.Replace('{{VERSION}}', $Version)
$releaseNotes = $releaseNotes.Replace('{{COMMIT_SHA}}', $CommitSha)
$releaseNotes = $releaseNotes.Replace('{{INSTALLER_NAME}}', $installerName)
if ($releaseNotes.Contains('{{') -or $releaseNotes.Contains('}}')) {
    throw "Windows release notes contain an unresolved template token."
}
[System.IO.File]::WriteAllText(
    (Join-Path $absoluteOutputDirectory 'release-notes.md'),
    $releaseNotes.Trim() + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

$artifactName = "ORIGAMI2-v$Version-windows-release-candidate"
$outputLines = @(
    "artifact_name=$artifactName",
    "installer_name=$installerName"
) -join "`n"
[System.IO.File]::AppendAllText(
    [System.IO.Path]::GetFullPath($OutputFile),
    $outputLines + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Packaged unsigned Windows release asset '$installerName' ($digest)."
