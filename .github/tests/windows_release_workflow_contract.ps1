[CmdletBinding()]
param(
    [string] $UnsignedExecutableFixture
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool] $Condition,
        [Parameter(Mandatory = $true)][string] $Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string] $Text,
        [Parameter(Mandatory = $true)][string] $Expected,
        [Parameter(Mandatory = $true)][string] $Message
    )

    Assert-True -Condition $Text.Contains($Expected) -Message $Message
}

function Invoke-ChildPowerShell {
    param(
        [Parameter(Mandatory = $true)][string] $Script,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )

    $powerShellPath = (Get-Process -Id $PID).Path
    $allArguments = @(
        '-NoLogo',
        '-NoProfile',
        '-NonInteractive',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        $Script
    ) + $Arguments
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $powerShellPath @allArguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Output = (($output | Out-String).Trim())
    }
}

$repositoryRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $PSScriptRoot '..\..')
)
$workflowPath = Join-Path $repositoryRoot '.github\workflows\release-windows.yml'
$validatorPath = Join-Path $repositoryRoot '.github\scripts\validate_windows_release.ps1'
$packagerPath = Join-Path $repositoryRoot '.github\scripts\package_windows_release.ps1'
$assetVerifierPath = Join-Path $repositoryRoot '.github\scripts\verify_windows_release_assets.ps1'
$packagePath = Join-Path $repositoryRoot 'apps\desktop\package.json'
$readinessPath = Join-Path $repositoryRoot '.github\release-readiness.json'
$releaseNotesTemplatePath = Join-Path (
    $repositoryRoot
) '.github\release-notes-windows.md'
$releaseNotesContractPath = Join-Path (
    $repositoryRoot
) '.github\release-notes-windows-contract.json'

$workflow = [System.IO.File]::ReadAllText($workflowPath)
$desktopPackage = [System.IO.File]::ReadAllText($packagePath) | ConvertFrom-Json
$readiness = [System.IO.File]::ReadAllText($readinessPath) | ConvertFrom-Json
$releaseNotesTemplate = [System.IO.File]::ReadAllText($releaseNotesTemplatePath)
$releaseNotesContract = [System.IO.File]::ReadAllText(
    $releaseNotesContractPath
) | ConvertFrom-Json

Assert-Contains $workflow 'workflow_dispatch:' 'Manual release trigger is missing.'
Assert-Contains $workflow 'tags:' 'Tag release trigger is missing.'
Assert-Contains $workflow '"v*"' 'Tag trigger must be followed by a strict runtime SemVer gate.'
Assert-Contains $workflow 'PUBLISH_UNSIGNED_WINDOWS_RELEASE' (
    'Manual unsigned-release acknowledgement is missing.'
)
Assert-Contains $workflow 'expected_commit:' 'Manual full commit confirmation is missing.'
Assert-Contains $workflow 'inputs.expected_commit || github.sha' (
    'Tag pushes must bind the expected commit to the triggering github.sha.'
)
Assert-Contains $workflow 'permissions: {}' 'Workflow default permissions must be empty.'
Assert-True (
    ([regex]::Matches($workflow, '(?m)^\s+contents: write\s*$')).Count -eq 1
) 'Exactly one publication job may receive contents: write.'
Assert-Contains $workflow 'persist-credentials: false' (
    'Checkout credentials must not persist in the workspace.'
)
Assert-Contains $workflow '--verify-tag' 'Release creation must require an existing tag.'
Assert-Contains $workflow '--draft' 'Release assets must first be staged in a draft.'
Assert-Contains $workflow '--draft=false' 'A fully uploaded draft must be explicitly published.'
Assert-Contains $workflow 'gh release view "$RELEASE_TAG"' (
    'The workflow must refuse an existing GitHub Release.'
)
Assert-Contains $workflow '--json isDraft' (
    'Existing draft releases must be detected and refused explicitly.'
)
Assert-Contains $workflow 'sha256sum --check --strict SHA256SUMS.txt' (
    'The publication job must independently check the installer digest.'
)
Assert-Contains $workflow 'run_instruction_export_offline_test.ps1' (
    'Formal release must generate instruction exports with outbound network blocked.'
)
Assert-Contains $workflow 'audit_instruction_exports.py' (
    'Formal release must independently parse generated instruction exports.'
)
Assert-Contains $workflow '--require-hashes --no-deps -r .github/release-audit-requirements.txt' (
    'Formal release must install only hash-pinned external instruction parsers.'
)
Assert-Contains $workflow 'python-version: "3.12.12"' 'Release Python must be patch-version pinned.'
Assert-Contains $workflow 'ref: ${{ needs.validate-test-build.outputs.commit_sha }}' (
    'Publication checkout must use the already validated immutable commit.'
)
Assert-Contains $workflow 'ORIGAMI2_INSTRUCTION_EXPORT_AUDIT_DIRECTORY' (
    'Formal release must use the bounded instruction export audit directory.'
)
Assert-Contains $workflow 'environment:' (
    'The publication job must expose a protectable GitHub environment gate.'
)
Assert-True (-not $workflow.Contains('pull_request:')) (
    'Release publication must never run for pull requests.'
)
Assert-True (-not $workflow.Contains('pull_request_target:')) (
    'Release publication must never run for pull_request_target.'
)
Assert-True (-not $workflow.Contains('${{ secrets.')) (
    'Release publication must not require repository secrets.'
)
Assert-True (-not $workflow.Contains('--clobber')) (
    'Release assets must never be overwritten.'
)
Assert-True (-not $workflow.ToLowerInvariant().Contains('macos-latest')) (
    'The formal release workflow must not publish a macOS build.'
)
Assert-True (
    $desktopPackage.scripts.'bundle:windows:ci' -ceq
        'tauri build --ci --no-sign --bundles nsis'
) 'The invoked package script must build only an unsigned NSIS installer.'
Assert-True (
    $readiness.schema -ceq 'origami2.windows-release-readiness.v1'
) 'The committed Windows release readiness schema is unsupported.'
foreach ($readinessFlag in @(
    $readiness.allMustRequirementsAccepted,
    $readiness.windowsOwnerE2eAccepted,
    $readiness.productionReleaseApproved
)) {
    Assert-True ($readinessFlag -is [bool]) (
        'Committed Windows release readiness flags must be booleans.'
    )
}
Assert-True (
    $releaseNotesContract.schema -ceq 'origami2.windows-release-notes.v1'
) 'The bilingual release notes contract schema is unsupported.'
$japaneseNotices = @($releaseNotesContract.requiredJapaneseNotices)
$englishNotices = @($releaseNotesContract.requiredEnglishNotices)
Assert-True ($japaneseNotices.Count -ge 3) (
    'Release notes must contract unsigned, checksum, and macOS notices in Japanese.'
)
Assert-True ($englishNotices.Count -ge 3) (
    'Release notes must contract unsigned, checksum, and macOS notices in English.'
)
foreach ($notice in $japaneseNotices) {
    Assert-True ($notice -is [string] -and $notice -cmatch '[^\x00-\x7f]') (
        'Each contracted Japanese release notice must contain Japanese text.'
    )
    Assert-Contains $releaseNotesTemplate $notice (
        'The release notes template is missing a contracted Japanese notice.'
    )
}
foreach ($notice in $englishNotices) {
    Assert-True ($notice -is [string] -and $notice -cnotmatch '[^\x00-\x7f]') (
        'Each contracted English release notice must remain ASCII.'
    )
    Assert-Contains $releaseNotesTemplate $notice (
        'The release notes template is missing a contracted English notice.'
    )
}

$usesLines = @(
    [regex]::Matches($workflow, '(?m)^\s*uses:\s*(?<value>\S+)\s*(?:#.*)?$') |
        ForEach-Object { $_.Groups['value'].Value }
)
Assert-True ($usesLines.Count -ge 1) 'Release workflow must contain pinned actions.'
foreach ($uses in $usesLines) {
    Assert-True ($uses -cmatch '@[0-9a-f]{40}$') (
        "Action '$uses' must be pinned to a full immutable commit SHA."
    )
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    'origami2-release-contract-' + [System.Guid]::NewGuid().ToString('N')
)
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null

try {
    $fixtureRoot = Join-Path $temporaryRoot 'repository'
    $tauriDirectory = Join-Path $fixtureRoot 'apps\desktop\src-tauri'
    New-Item -ItemType Directory -Path $tauriDirectory -Force | Out-Null
    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'Cargo.toml'),
        "[workspace]`n`n[workspace.package]`nversion = `"0.1.0`"`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'Cargo.lock'),
        "version = 4`n`n[[package]]`nname = `"origami2-desktop`"`nversion = `"0.1.0`"`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        (Join-Path $tauriDirectory 'tauri.conf.json'),
        "{`"version`":`"0.1.0`"}`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    $fixtureReadinessPath = Join-Path $fixtureRoot '.github\release-readiness.json'
    New-Item -ItemType Directory -Path (
        Split-Path -Parent $fixtureReadinessPath
    ) | Out-Null
    [System.IO.File]::WriteAllText(
        $fixtureReadinessPath,
        (
            '{' +
            '"schema":"origami2.windows-release-readiness.v1",' +
            '"version":"0.1.0",' +
            '"allMustRequirementsAccepted":false,' +
            '"windowsOwnerE2eAccepted":false,' +
            '"productionReleaseApproved":false' +
            "}`n"
        ),
        [System.Text.UTF8Encoding]::new($false)
    )

    & git -C $fixtureRoot init --initial-branch=main | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not initialize release contract fixture.' }
    & git -C $fixtureRoot config core.autocrlf false | Out-Null
    & git -C $fixtureRoot config user.name 'ORIGAMI2 release contract' | Out-Null
    & git -C $fixtureRoot config user.email 'release-contract@invalid.example' | Out-Null
    & git -C $fixtureRoot add -- `
        Cargo.toml `
        Cargo.lock `
        apps/desktop/src-tauri/tauri.conf.json `
        .github/release-readiness.json
    & git -C $fixtureRoot commit -m 'unapproved fixture' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not commit release contract fixture.' }
    & git -C $fixtureRoot tag v0.1.0

    $unapprovedCommit = (& git -C $fixtureRoot rev-parse HEAD).Trim()
    $unapproved = Invoke-ChildPowerShell -Script $validatorPath -Arguments @(
        '-RepositoryRoot', $fixtureRoot,
        '-EventName', 'workflow_dispatch',
        '-Tag', 'v0.1.0',
        '-ExpectedCommit', $unapprovedCommit,
        '-Confirmation', 'PUBLISH_UNSIGNED_WINDOWS_RELEASE',
        '-OutputFile', (Join-Path $temporaryRoot 'unapproved.txt')
    )
    Assert-True ($unapproved.ExitCode -ne 0) (
        'Release gate accepted a build before all readiness approvals.'
    )

    $fixtureCargo = [System.IO.File]::ReadAllText(
        (Join-Path $fixtureRoot 'Cargo.toml')
    ).Replace('0.1.0', '0.1.1')
    $fixtureLock = [System.IO.File]::ReadAllText(
        (Join-Path $fixtureRoot 'Cargo.lock')
    ).Replace('0.1.0', '0.1.1')
    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'Cargo.toml'),
        $fixtureCargo,
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        (Join-Path $fixtureRoot 'Cargo.lock'),
        $fixtureLock,
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        (Join-Path $tauriDirectory 'tauri.conf.json'),
        "{`"version`":`"0.1.1`"}`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        $fixtureReadinessPath,
        (
            '{' +
            '"schema":"origami2.windows-release-readiness.v1",' +
            '"version":"0.1.1",' +
            '"allMustRequirementsAccepted":true,' +
            '"windowsOwnerE2eAccepted":true,' +
            '"productionReleaseApproved":true' +
            "}`n"
        ),
        [System.Text.UTF8Encoding]::new($false)
    )
    & git -C $fixtureRoot add -- `
        Cargo.toml `
        Cargo.lock `
        apps/desktop/src-tauri/tauri.conf.json `
        .github/release-readiness.json
    & git -C $fixtureRoot commit -m 'approved fixture' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not commit approved release fixture.' }
    & git -C $fixtureRoot tag v0.1.1
    & git -C $fixtureRoot tag v0.1.2
    $fixtureCommit = (& git -C $fixtureRoot rev-parse HEAD).Trim()

    $gateOutput = Join-Path $temporaryRoot 'gate-output.txt'
    $success = Invoke-ChildPowerShell -Script $validatorPath -Arguments @(
        '-RepositoryRoot', $fixtureRoot,
        '-EventName', 'workflow_dispatch',
        '-Tag', 'v0.1.1',
        '-ExpectedCommit', $fixtureCommit,
        '-Confirmation', 'PUBLISH_UNSIGNED_WINDOWS_RELEASE',
        '-OutputFile', $gateOutput
    )
    Assert-True ($success.ExitCode -eq 0) (
        "Valid release gate fixture failed: $($success.Output)"
    )
    $gateText = [System.IO.File]::ReadAllText($gateOutput)
    Assert-Contains $gateText 'release_tag=v0.1.1' 'Gate did not emit the validated tag.'
    Assert-Contains $gateText "commit_sha=$fixtureCommit" 'Gate did not emit the validated commit.'

    $wrongConfirmation = Invoke-ChildPowerShell -Script $validatorPath -Arguments @(
        '-RepositoryRoot', $fixtureRoot,
        '-EventName', 'workflow_dispatch',
        '-Tag', 'v0.1.1',
        '-ExpectedCommit', $fixtureCommit,
        '-Confirmation', 'DO_NOT_PUBLISH',
        '-OutputFile', (Join-Path $temporaryRoot 'wrong-confirmation.txt')
    )
    Assert-True ($wrongConfirmation.ExitCode -ne 0) (
        'Release gate accepted a missing unsigned-release acknowledgement.'
    )

    $versionMismatch = Invoke-ChildPowerShell -Script $validatorPath -Arguments @(
        '-RepositoryRoot', $fixtureRoot,
        '-EventName', 'workflow_dispatch',
        '-Tag', 'v0.1.2',
        '-ExpectedCommit', $fixtureCommit,
        '-Confirmation', 'PUBLISH_UNSIGNED_WINDOWS_RELEASE',
        '-OutputFile', (Join-Path $temporaryRoot 'version-mismatch.txt')
    )
    Assert-True ($versionMismatch.ExitCode -ne 0) (
        'Release gate accepted a tag that differs from application versions.'
    )

    if ([string]::IsNullOrWhiteSpace($UnsignedExecutableFixture)) {
        $fixtureCandidates = @(
            Get-ChildItem -LiteralPath (
                Join-Path $repositoryRoot 'target\release\bundle\nsis'
            ) -File -Filter '*.exe' -ErrorAction SilentlyContinue
        ) + @(
            Get-Item -LiteralPath (
                Join-Path $repositoryRoot 'target\debug\origami2-desktop.exe'
            ) -ErrorAction SilentlyContinue
        )
        if ($fixtureCandidates.Count -ne 1) {
            throw (
                'Provide -UnsignedExecutableFixture or make exactly one known unsigned ' +
                'ORIGAMI2 executable available.'
            )
        }
        $UnsignedExecutableFixture = $fixtureCandidates[0].FullName
    }
    $resolvedUnsignedFixture = (Resolve-Path -LiteralPath $UnsignedExecutableFixture).Path
    Assert-True (
        (Get-AuthenticodeSignature -LiteralPath $resolvedUnsignedFixture).Status -eq
            [System.Management.Automation.SignatureStatus]::NotSigned
    ) 'The release contract fixture executable must be unsigned.'

    $bundleDirectory = Join-Path $temporaryRoot 'bundle'
    $releaseDirectory = Join-Path $temporaryRoot 'release'
    New-Item -ItemType Directory -Path $bundleDirectory | Out-Null
    $fakeInstaller = Join-Path $bundleDirectory 'fixture-setup.exe'
    Copy-Item -LiteralPath $resolvedUnsignedFixture -Destination $fakeInstaller
    $packageOutput = Join-Path $temporaryRoot 'package-output.txt'

    & $packagerPath `
        -BundleDirectory $bundleDirectory `
        -OutputDirectory $releaseDirectory `
        -Version '0.1.0' `
        -CommitSha $fixtureCommit `
        -OutputFile $packageOutput | Out-Null
    & $assetVerifierPath `
        -ReleaseDirectory $releaseDirectory `
        -Version '0.1.0' `
        -CommitSha $fixtureCommit `
        -RequireUnsigned | Out-Null

    $packagedInstaller = Join-Path $releaseDirectory (
        'ORIGAMI2-v0.1.0-windows-x64-unsigned-setup.exe'
    )
    [System.IO.File]::AppendAllText($packagedInstaller, 'tamper')
    $tamperRejected = $false
    try {
        & $assetVerifierPath `
            -ReleaseDirectory $releaseDirectory `
            -Version '0.1.0' `
            -CommitSha $fixtureCommit `
            -RequireUnsigned | Out-Null
    } catch {
        $tamperRejected = $true
    }
    Assert-True $tamperRejected 'Release asset verifier accepted a tampered installer.'
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        $resolvedTemporaryRoot = (Resolve-Path -LiteralPath $temporaryRoot).Path
        $resolvedSystemTemp = (Resolve-Path -LiteralPath (
            [System.IO.Path]::GetTempPath()
        )).Path.TrimEnd(
            [System.IO.Path]::DirectorySeparatorChar,
            [System.IO.Path]::AltDirectorySeparatorChar
        )
        Assert-True (
            $resolvedTemporaryRoot.StartsWith(
                $resolvedSystemTemp + [System.IO.Path]::DirectorySeparatorChar,
                [System.StringComparison]::OrdinalIgnoreCase
            )
        ) 'Refusing to remove an unexpected release contract fixture path.'
        Remove-Item -LiteralPath $resolvedTemporaryRoot -Recurse -Force
    }
}

# Expected rejection fixtures above intentionally leave a child pwsh exit code of 1.
# GitHub Actions appends a shell footer that exits with the last native exit code,
# even after this contract has completed successfully, so clear that captured code.
$global:LASTEXITCODE = 0
Write-Output 'Windows production release workflow contract passed.'
