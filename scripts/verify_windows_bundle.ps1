[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $BundleDirectory,

    [string] $ExpectedVersion = '',

    [ValidateSet('Ignore', 'NotSigned', 'Valid')]
    [string] $ExpectedSignatureStatus = 'Ignore',

    [string] $PortableExecutable = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$resolvedBundleDirectory = (Resolve-Path -LiteralPath $BundleDirectory).Path
$bundleEntries = @(Get-ChildItem -LiteralPath $resolvedBundleDirectory -Force)
if ($bundleEntries.Count -ne 1) {
    throw "[bundle-layout] Bundle directory must contain exactly one installer, found $($bundleEntries.Count) entries."
}
$installers = @(Get-ChildItem -LiteralPath $resolvedBundleDirectory -File -Filter '*.exe')
if ($installers.Count -ne 1) {
    throw "[bundle-layout] Expected exactly one NSIS installer, found $($installers.Count)."
}

function Assert-BoundedRegularFile {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Label
    )

    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "[file-identity] $Label must not be a reparse point."
    }
    if ($item.Length -le 0 -or $item.Length -gt 536870912) {
        throw "[file-identity] $Label size is outside the 512 MiB audit bound."
    }
    $hardLinks = @(& fsutil.exe hardlink list $item.FullName 2>$null)
    if ($LASTEXITCODE -ne 0 -or $hardLinks.Count -ne 1) {
        throw "[file-identity] $Label must have exactly one hard link."
    }
}

Assert-BoundedRegularFile -Path $installers[0].FullName -Label 'Windows NSIS installer'

function Assert-Version {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Label
    )

    if ([string]::IsNullOrWhiteSpace($ExpectedVersion)) {
        return
    }
    if ($ExpectedVersion -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
        throw "[version] Expected version must be canonical stable SemVer."
    }
    $info = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
    $candidates = @($info.ProductVersion, $info.FileVersion) |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    $escaped = [regex]::Escape($ExpectedVersion)
    if (-not ($candidates | Where-Object { $_ -cmatch "^$escaped(?:\.0)?$" })) {
        throw "[version] $Label version does not match $ExpectedVersion."
    }
}

function Assert-Signature {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Label
    )

    if ($ExpectedSignatureStatus -eq 'Ignore') {
        return
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $actual = $signature.Status.ToString()
    if ($actual -cne $ExpectedSignatureStatus) {
        throw "[signature] $Label Authenticode status is '$actual', expected '$ExpectedSignatureStatus'."
    }
    if ($ExpectedSignatureStatus -eq 'Valid') {
        if ($null -eq $signature.SignerCertificate) {
            throw "[signature] $Label has no Authenticode signer certificate."
        }
        if ($null -eq $signature.TimeStamperCertificate) {
            throw "[signature] $Label has no trusted RFC 3161 timestamp certificate."
        }
        $timestampStart = $signature.TimeStamperCertificate.NotBefore.ToUniversalTime()
        $timestampEnd = $signature.TimeStamperCertificate.NotAfter.ToUniversalTime()
        if ($timestampEnd -le $timestampStart) {
            throw "[signature] $Label timestamp certificate validity is invalid."
        }
    }
}

Assert-Version -Path $installers[0].FullName -Label 'Windows NSIS installer'
Assert-Signature -Path $installers[0].FullName -Label 'Windows NSIS installer'
if (-not [string]::IsNullOrWhiteSpace($PortableExecutable)) {
    $resolvedPortable = (Resolve-Path -LiteralPath $PortableExecutable).Path
    Assert-BoundedRegularFile -Path $resolvedPortable -Label 'Windows portable executable'
    Assert-Version -Path $resolvedPortable -Label 'Windows portable executable'
    Assert-Signature -Path $resolvedPortable -Label 'Windows portable executable'
}

$sevenZipCommand = Get-Command '7z.exe' -ErrorAction SilentlyContinue
if ($null -ne $sevenZipCommand) {
    $sevenZip = $sevenZipCommand.Source
} else {
    $sevenZip = Join-Path $env:ProgramFiles '7-Zip\7z.exe'
}
if (-not (Test-Path -LiteralPath $sevenZip -PathType Leaf)) {
    throw "[archive-tool] 7-Zip is required to inspect the generated NSIS installer."
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    'origami2-nsis-audit-' + [System.Guid]::NewGuid().ToString('N')
)
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null

try {
    $outputOption = "-o$temporaryRoot"
    & $sevenZip x $installers[0].FullName $outputOption -y | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "[archive-extract] 7-Zip could not extract the NSIS installer."
    }

    $fontFiles = @(
        Get-ChildItem -LiteralPath $temporaryRoot -Recurse -File -Filter 'NotoSansJP-Variable.ttf'
    )
    $licenseFiles = @(
        Get-ChildItem -LiteralPath $temporaryRoot -Recurse -File -Filter 'NotoSansJP-OFL.txt'
    )
    $appExecutables = @(
        Get-ChildItem -LiteralPath $temporaryRoot -Recurse -File -Filter 'origami2-desktop.exe'
    )
    $extractedFiles = @(Get-ChildItem -LiteralPath $temporaryRoot -Recurse -File -Force)
    if ($extractedFiles.Count -gt 100000) {
        throw "[archive-bounds] Extracted installer exceeds the 100000-file audit bound."
    }
    $extractedBytes = 0L
    foreach ($file in $extractedFiles) {
        if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "[archive-bounds] Extracted installer contains a reparse point."
        }
        if ($file.Length -gt 536870912) {
            throw "[archive-bounds] Extracted installer contains an oversized file."
        }
        $extractedBytes += $file.Length
        if ($extractedBytes -gt 1073741824) {
            throw "[archive-bounds] Extracted installer exceeds the 1 GiB audit bound."
        }
    }

    if ($fontFiles.Count -ne 1) {
        throw "[resource] Expected exactly one bundled Noto Sans JP font, found $($fontFiles.Count)."
    }
    if ($licenseFiles.Count -ne 1) {
        throw "[resource] Expected exactly one bundled Noto Sans JP license, found $($licenseFiles.Count)."
    }
    if ($appExecutables.Count -ne 1) {
        throw "[resource] Expected exactly one bundled origami2-desktop executable, found $($appExecutables.Count)."
    }
    Assert-Version -Path $appExecutables[0].FullName -Label 'Embedded Windows executable'
    Assert-Signature -Path $appExecutables[0].FullName -Label 'Embedded Windows executable'
    if (-not [string]::IsNullOrWhiteSpace($PortableExecutable)) {
        $portableHash = (Get-FileHash -LiteralPath $resolvedPortable -Algorithm SHA256).Hash
        $embeddedHash = (Get-FileHash -LiteralPath $appExecutables[0].FullName -Algorithm SHA256).Hash
        if ($portableHash -cne $embeddedHash) {
            throw '[payload-identity] Portable and embedded Windows executable payloads differ.'
        }
    }
    if ($fontFiles[0].Directory.Name -cne 'fonts') {
        throw "[resource] Bundled Noto Sans JP font is not in the expected fonts directory."
    }
    if ($licenseFiles[0].Directory.Name -cne 'licenses') {
        throw "[resource] Bundled Noto Sans JP license is not in the expected licenses directory."
    }

    $expectedFontHash = 'C2F3B4D463500A2DDCD3849CDED1FCEEB9FD6D1C32E6CBECD568453BA50FC68F'
    $expectedLicenseHash = '1C05C68C34F9708415AADA51F17E1B0092D2CEA709BF4A94CD38114F9E73D7D9'
    $fontHash = (Get-FileHash -LiteralPath $fontFiles[0].FullName -Algorithm SHA256).Hash
    $licenseHash = (Get-FileHash -LiteralPath $licenseFiles[0].FullName -Algorithm SHA256).Hash

    if ($fontHash -cne $expectedFontHash) {
        throw "[resource-integrity] Bundled Noto Sans JP font digest differs: $fontHash."
    }
    if ($licenseHash -cne $expectedLicenseHash) {
        throw "[resource-integrity] Bundled Noto Sans JP license digest differs: $licenseHash."
    }

    Write-Output (
        "Windows NSIS bundle audit passed: {0} ({1} bytes)." -f
        $installers[0].Name,
        $installers[0].Length
    )
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        $resolvedTemporaryRoot = (Resolve-Path -LiteralPath $temporaryRoot).Path
        $resolvedSystemTemp = (Resolve-Path -LiteralPath ([System.IO.Path]::GetTempPath())).Path
        $resolvedSystemTemp = $resolvedSystemTemp.TrimEnd(
            [System.IO.Path]::DirectorySeparatorChar,
            [System.IO.Path]::AltDirectorySeparatorChar
        )
        if (-not $resolvedTemporaryRoot.StartsWith(
            $resolvedSystemTemp + [System.IO.Path]::DirectorySeparatorChar,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
            throw "[cleanup-boundary] Refusing to remove an unexpected temporary path."
        }
        Remove-Item -LiteralPath $resolvedTemporaryRoot -Recurse -Force
    }
}
