[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$auditDirectory = $env:ORIGAMI2_INSTRUCTION_EXPORT_AUDIT_DIRECTORY
if ([string]::IsNullOrWhiteSpace($auditDirectory)) {
    throw 'ORIGAMI2_INSTRUCTION_EXPORT_AUDIT_DIRECTORY is required.'
}
New-Item -ItemType Directory -Path $auditDirectory -Force | Out-Null

$cargoErrorLog = Join-Path ([System.IO.Path]::GetTempPath()) (
    'origami2-cargo-test-' + [System.Guid]::NewGuid().ToString('N') + '.log'
)
try {
    $cargoMessages = @(
        & cargo test -p ori-formats --locked --no-run --message-format=json 2> $cargoErrorLog
    )
    if ($LASTEXITCODE -ne 0) {
        Get-Content -LiteralPath $cargoErrorLog | Write-Host
        throw 'Could not build the ori-formats test executable.'
    }

    $testArtifacts = @(
        $cargoMessages |
            ForEach-Object {
                try {
                    $_ | ConvertFrom-Json -ErrorAction Stop
                } catch {
                    $null
                }
            } |
            Where-Object {
                $_.reason -eq 'compiler-artifact' -and
                $_.profile.test -eq $true -and
                $_.target.name -eq 'ori_formats' -and
                $_.target.kind -contains 'lib' -and
                $null -ne $_.executable
            }
    )
    if ($testArtifacts.Count -ne 1) {
        throw "Expected exactly one ori-formats library test executable, found $($testArtifacts.Count)."
    }

    $testExecutable = (Resolve-Path -LiteralPath $testArtifacts[0].executable).Path
    $disabledFirewallProfiles = @(Get-NetFirewallProfile | Where-Object { -not $_.Enabled })
    if ($disabledFirewallProfiles.Count -ne 0) {
        $profileNames = ($disabledFirewallProfiles | ForEach-Object { $_.Name }) -join ', '
        throw "Windows Firewall must be enabled for the offline test; disabled profiles: $profileNames."
    }

    $firewallRuleName = 'ORIGAMI2 instruction export offline ' + [System.Guid]::NewGuid().ToString('N')
    $firewallRule = $null
    try {
        $firewallRule = New-NetFirewallRule `
            -DisplayName $firewallRuleName `
            -Direction Outbound `
            -Program $testExecutable `
            -Action Block `
            -Profile Any `
            -Enabled True

        $activeRule = Get-NetFirewallRule -PolicyStore ActiveStore -Name $firewallRule.Name
        $applicationFilter = $activeRule | Get-NetFirewallApplicationFilter
        if (
            $activeRule.Direction.ToString() -cne 'Outbound' -or
            $activeRule.Action.ToString() -cne 'Block' -or
            $activeRule.Enabled.ToString() -cne 'True' -or
            -not [string]::Equals(
                $applicationFilter.Program,
                $testExecutable,
                [System.StringComparison]::OrdinalIgnoreCase
            )
        ) {
            throw 'The outbound block rule was not activated for the test executable.'
        }

        & $testExecutable `
            'instruction_export::tests::both_formats_are_deterministic_and_report_complete_metadata' `
            --exact `
            --nocapture
        if ($LASTEXITCODE -ne 0) {
            throw 'Instruction export failed while outbound network access was blocked.'
        }
    } finally {
        if ($null -ne $firewallRule) {
            Remove-NetFirewallRule -Name $firewallRule.Name
        }
    }
} finally {
    if (Test-Path -LiteralPath $cargoErrorLog) {
        Remove-Item -LiteralPath $cargoErrorLog -Force
    }
}
