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
$leaf = $leafCertificates[0]
$now = [DateTime]::UtcNow
if ($leaf.NotBefore.ToUniversalTime() -gt $now -or $leaf.NotAfter.ToUniversalTime() -lt $now) {
    throw 'Windows signing certificate is not currently valid.'
}
$strongSignatureAlgorithms = @(
    '1.2.840.113549.1.1.11',
    '1.2.840.113549.1.1.12',
    '1.2.840.113549.1.1.13',
    '1.2.840.10045.4.3.2',
    '1.2.840.10045.4.3.3',
    '1.2.840.10045.4.3.4'
)
if (-not $strongSignatureAlgorithms.Contains($leaf.SignatureAlgorithm.Value)) {
    throw 'Windows signing certificate uses a weak or unsupported signature algorithm.'
}
$rsa = [Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($leaf)
$ecdsa = [Security.Cryptography.X509Certificates.ECDsaCertificateExtensions]::GetECDsaPublicKey($leaf)
try {
    if (($null -eq $rsa -or $rsa.KeySize -lt 2048) -and
        ($null -eq $ecdsa -or $ecdsa.KeySize -lt 256)) {
        throw 'Windows signing certificate public key is too weak.'
    }
} finally {
    if ($null -ne $rsa) { $rsa.Dispose() }
    if ($null -ne $ecdsa) { $ecdsa.Dispose() }
}
$chain = [Security.Cryptography.X509Certificates.X509Chain]::new()
try {
    $chain.ChainPolicy.RevocationMode = [Security.Cryptography.X509Certificates.X509RevocationMode]::Online
    $chain.ChainPolicy.RevocationFlag = [Security.Cryptography.X509Certificates.X509RevocationFlag]::EntireChain
    $chain.ChainPolicy.VerificationFlags = [Security.Cryptography.X509Certificates.X509VerificationFlags]::NoFlag
    $chain.ChainPolicy.UrlRetrievalTimeout = [TimeSpan]::FromSeconds(30)
    foreach ($certificateInPfx in @($pfx.OtherCertificates)) {
        [void] $chain.ChainPolicy.ExtraStore.Add($certificateInPfx)
    }
    if (-not $chain.Build($leaf)) {
        throw 'Windows signing certificate chain or revocation policy validation failed.'
    }
} finally {
    $chain.Dispose()
}

$signature = Get-AuthenticodeSignature -LiteralPath $File
if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid -or
    $null -eq $signature.SignerCertificate -or
    $null -eq $signature.TimeStamperCertificate) {
    throw 'Windows Authenticode signature, chain, or timestamp is invalid.'
}
if ($signature.SignerCertificate.Thumbprint -cne $leaf.Thumbprint) {
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
if (-not $strongSignatureAlgorithms.Contains($signature.TimeStamperCertificate.SignatureAlgorithm.Value)) {
    throw 'Windows timestamp certificate uses a weak or unsupported signature algorithm.'
}

$verificationLog = Join-Path $env:RUNNER_TEMP 'windows-signature-verification.log'
try {
    & signtool verify /v /pa /all /tw $File *> $verificationLog
    if ($LASTEXITCODE -ne 0) {
        throw 'Windows Authenticode policy, chain, or RFC 3161 timestamp verification failed.'
    }
    $timestampLines = @(Get-Content -LiteralPath $verificationLog | Where-Object {
        $_ -match '^\s*(?:The signature is timestamped:|Timestamp:)\s*(.+?)\s*$'
    })
    if ($timestampLines.Count -ne 1 -or
        $timestampLines[0] -notmatch '^\s*(?:The signature is timestamped:|Timestamp:)\s*(.+?)\s*$') {
        throw 'Windows Authenticode timestamp evidence is missing or ambiguous.'
    }
    $timestamp = [DateTimeOffset]::Parse(
        $Matches[1],
        [Globalization.CultureInfo]::CurrentCulture,
        [Globalization.DateTimeStyles]::AssumeLocal
    ).ToUniversalTime()
    $verificationTime = [DateTimeOffset]::UtcNow
    if ($timestamp -gt $verificationTime.AddMinutes(5) -or
        $timestamp -lt $verificationTime.AddHours(-1)) {
        throw 'Windows Authenticode timestamp is outside the release build window.'
    }
} finally {
    if (Test-Path -LiteralPath $verificationLog) {
        Remove-Item -LiteralPath $verificationLog -Force -ErrorAction Stop
    }
}
Write-Output 'Verified Windows Authenticode identity, chain, and RFC 3161 timestamp.'
