[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $File,
    [Parameter(Mandatory = $true)][string] $Certificate
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$passwordText = $env:WINDOWS_CERTIFICATE_PASSWORD
if ([string]::IsNullOrWhiteSpace($passwordText)) {
    throw 'Windows signing certificate password is unavailable.'
}
$password = ConvertTo-SecureString -String $passwordText -AsPlainText -Force
$pfx = Get-PfxData -FilePath $Certificate -Password $password
$leafCertificates = @($pfx.EndEntityCertificates)
if ($leafCertificates.Count -ne 1) {
    throw 'Windows signing PFX must contain exactly one leaf certificate.'
}

$signature = Get-AuthenticodeSignature -LiteralPath $File
if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid -or
    $null -eq $signature.SignerCertificate -or
    $null -eq $signature.TimeStamperCertificate) {
    throw 'Windows Authenticode signature, chain, or timestamp is invalid.'
}
if ($signature.SignerCertificate.Thumbprint -cne $leafCertificates[0].Thumbprint) {
    throw 'Windows Authenticode leaf certificate does not match the configured PFX.'
}
$codeSigningOid = '1.3.6.1.5.5.7.3.3'
$timestampingOid = '1.3.6.1.5.5.7.3.8'
if (-not @($signature.SignerCertificate.EnhancedKeyUsageList).ObjectId.Value.Contains($codeSigningOid)) {
    throw 'Windows Authenticode leaf certificate lacks code-signing usage.'
}
if (-not @($signature.TimeStamperCertificate.EnhancedKeyUsageList).ObjectId.Value.Contains($timestampingOid)) {
    throw 'Windows Authenticode timestamp certificate lacks timestamping usage.'
}

$verificationLog = Join-Path $env:RUNNER_TEMP 'windows-signature-verification.log'
try {
    & signtool verify /q /pa /all /tw $File *> $verificationLog
    if ($LASTEXITCODE -ne 0) {
        throw 'Windows Authenticode policy, chain, or RFC 3161 timestamp verification failed.'
    }
} finally {
    Remove-Item -LiteralPath $verificationLog -Force -ErrorAction SilentlyContinue
}
Write-Output 'Verified Windows Authenticode identity, chain, and RFC 3161 timestamp.'
