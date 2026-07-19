[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $RepositoryRoot,

    [Parameter(Mandatory = $true)]
    [ValidateSet('push', 'workflow_dispatch')]
    [string] $EventName,

    [Parameter(Mandatory = $true)]
    [string] $Tag,

    [Parameter(Mandatory = $true)]
    [string] $ExpectedCommit,

    [Parameter(Mandatory = $true)]
    [string] $Confirmation,

    [Parameter(Mandatory = $true)]
    [string] $OutputFile
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Read-ExactText {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required release input '$Path' does not exist."
    }
    return [System.IO.File]::ReadAllText((Resolve-Path -LiteralPath $Path).Path)
}

function Invoke-GitText {
    param(
        [Parameter(Mandatory = $true)][string] $WorkingDirectory,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )

    $output = & git -C $WorkingDirectory @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Git command failed: git $($Arguments -join ' ')"
    }
    return (($output | Out-String).Trim())
}

$resolvedRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
if ($Tag -cnotmatch '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw "Release tag must be canonical stable SemVer in the form vMAJOR.MINOR.PATCH."
}
$version = $Tag.Substring(1)

if ($ExpectedCommit -cnotmatch '^[0-9a-f]{40}$') {
    throw "Expected commit must be a full lowercase 40-character Git SHA."
}
if ($Confirmation -cne 'PUBLISH_UNSIGNED_WINDOWS_RELEASE') {
    throw "Unsigned Windows release confirmation was not provided."
}
if ($EventName -eq 'push' -and $env:GITHUB_REF_TYPE -and $env:GITHUB_REF_TYPE -cne 'tag') {
    throw "Push-triggered publication is only allowed for a tag ref."
}

$headCommit = Invoke-GitText -WorkingDirectory $resolvedRoot -Arguments @(
    'rev-parse',
    '--verify',
    'HEAD^{commit}'
)
$tagCommit = Invoke-GitText -WorkingDirectory $resolvedRoot -Arguments @(
    'rev-parse',
    '--verify',
    "refs/tags/$Tag^{commit}"
)
if ($headCommit -cne $ExpectedCommit) {
    throw "Checked-out commit does not match the trigger's expected commit."
}
if ($tagCommit -cne $ExpectedCommit) {
    throw "Release tag does not resolve to the checked-out expected commit."
}

$status = Invoke-GitText -WorkingDirectory $resolvedRoot -Arguments @(
    'status',
    '--porcelain=v1',
    '--untracked-files=no'
)
if ($status.Length -ne 0) {
    throw "Tracked files are modified before release validation."
}

$tauriPath = Join-Path $resolvedRoot 'apps\desktop\src-tauri\tauri.conf.json'
$tauriText = Read-ExactText -Path $tauriPath
try {
    $tauriConfig = $tauriText | ConvertFrom-Json
} catch {
    throw "Tauri configuration is not valid JSON."
}
if (-not ($tauriConfig.version -is [string]) -or $tauriConfig.version -cne $version) {
    throw "Tauri version must exactly match release tag $Tag."
}

$cargoPath = Join-Path $resolvedRoot 'Cargo.toml'
$cargoText = Read-ExactText -Path $cargoPath
$workspaceMatch = [regex]::Match(
    $cargoText,
    '(?ms)^\[workspace\.package\]\s*(?<body>.*?)(?=^\[|\z)'
)
if (-not $workspaceMatch.Success) {
    throw "Cargo workspace.package section is missing."
}
$workspaceVersions = [regex]::Matches(
    $workspaceMatch.Groups['body'].Value,
    '(?m)^version\s*=\s*"(?<version>[^"]+)"\s*$'
)
if ($workspaceVersions.Count -ne 1 -or
    $workspaceVersions[0].Groups['version'].Value -cne $version) {
    throw "Cargo workspace version must exactly match release tag $Tag."
}

$lockPath = Join-Path $resolvedRoot 'Cargo.lock'
$lockText = Read-ExactText -Path $lockPath
$desktopPackages = [regex]::Matches(
    $lockText,
    '(?ms)^\[\[package\]\]\s*name\s*=\s*"origami2-desktop"\s*' +
        'version\s*=\s*"(?<version>[^"]+)"'
)
if ($desktopPackages.Count -ne 1 -or
    $desktopPackages[0].Groups['version'].Value -cne $version) {
    throw "Locked desktop package version must exactly match release tag $Tag."
}

$readinessPath = Join-Path $resolvedRoot '.github\release-readiness.json'
$readinessText = Read-ExactText -Path $readinessPath
try {
    $readiness = $readinessText | ConvertFrom-Json
} catch {
    throw "Windows production release readiness file is not valid JSON."
}
$readinessProperties = @($readiness.PSObject.Properties)
$expectedReadinessProperties = @(
    'schema',
    'version',
    'allMustRequirementsAccepted',
    'windowsOwnerE2eAccepted',
    'productionReleaseApproved'
)
$actualReadinessPropertyNames = (
    $readinessProperties.Name | Sort-Object
) -join "`n"
$expectedReadinessPropertyNames = (
    $expectedReadinessProperties | Sort-Object
) -join "`n"
if ($actualReadinessPropertyNames -cne $expectedReadinessPropertyNames) {
    throw "Windows production release readiness file has an unexpected shape."
}
if ($readiness.schema -cne 'origami2.windows-release-readiness.v1') {
    throw "Windows production release readiness schema is unsupported."
}
if ($readiness.version -cne $version) {
    throw "Windows production release readiness version must match the tag."
}
$readinessFlags = @(
    $readiness.allMustRequirementsAccepted,
    $readiness.windowsOwnerE2eAccepted,
    $readiness.productionReleaseApproved
)
foreach ($flag in $readinessFlags) {
    if ($flag -isnot [bool]) {
        throw "Windows production release readiness flags must be booleans."
    }
}
if ($readinessFlags -contains $false) {
    throw (
        "Windows production release is not approved: all MUST requirements, " +
        "owner E2E, and explicit publication approval must all be accepted."
    )
}

$outputLines = @(
    "release_tag=$Tag",
    "version=$version",
    "commit_sha=$headCommit"
) -join "`n"
[System.IO.File]::AppendAllText(
    [System.IO.Path]::GetFullPath($OutputFile),
    $outputLines + "`n",
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Windows release gate passed for $Tag at $headCommit."
