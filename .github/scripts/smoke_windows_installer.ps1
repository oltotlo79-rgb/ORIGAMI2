param(
  [Parameter(Mandatory = $true)][string]$BundleDirectory,
  [int]$TimeoutSeconds = 90
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-FullPath([string]$Path) {
  $full = [IO.Path]::GetFullPath($Path)
  $root = [IO.Path]::GetPathRoot($full)
  if ([string]::Equals($full, $root, [StringComparison]::OrdinalIgnoreCase)) { return $root }
  return $full.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
}
function Test-DirectChild([string]$Child, [string]$Parent) {
  [string]::Equals((Get-FullPath (Split-Path -Parent $Child)), (Get-FullPath $Parent), [StringComparison]::OrdinalIgnoreCase)
}
function Assert-SafeDirectChild([string]$Child, [string]$Parent) {
  if (-not (Test-DirectChild $Child $Parent)) { throw 'Installer smoke root must be a direct child of RUNNER_TEMP.' }
  $parentItem = Get-Item -LiteralPath $Parent -Force
  if (($parentItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'RUNNER_TEMP must not be a reparse point.' }
}
function Assert-NoReparse([string]$Root) {
  if (-not (Test-Path -LiteralPath $Root)) { return }
  $pending = [Collections.Generic.Stack[string]]::new()
  $pending.Push((Get-FullPath $Root))
  while ($pending.Count -gt 0) {
    $directory = Get-Item -LiteralPath $pending.Pop() -Force
    if (($directory.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "Installer smoke path contains a reparse point: $($directory.FullName)"
    }
    $children = @(Get-ChildItem -LiteralPath $directory.FullName -Force)
    foreach ($item in $children) {
      if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Installer smoke path contains a reparse point: $($item.FullName)"
      }
      if ($item.PSIsContainer) { $pending.Push($item.FullName) }
    }
  }
}
function Invoke-Bounded([string]$File, [string[]]$Arguments, [int]$Seconds) {
  $start = [Diagnostics.ProcessStartInfo]::new()
  $start.FileName = $File
  $start.UseShellExecute = $false
  $start.CreateNoWindow = $true
  foreach ($argument in $Arguments) { [void]$start.ArgumentList.Add($argument) }
  $process = [Diagnostics.Process]::new()
  $process.StartInfo = $start
  if (-not $process.Start()) { throw 'Could not start installer smoke process.' }
  if (-not $process.WaitForExit($Seconds * 1000)) {
    try { $process.Kill($true) } catch { & taskkill.exe /PID $process.Id /T /F 2>$null | Out-Null }
    if (-not $process.WaitForExit(10000)) { throw 'Timed-out installer process could not be terminated; cleanup is unsafe.' }
    throw "Installer smoke process timed out: $([IO.Path]::GetFileName($File))"
  }
  if ($process.ExitCode -ne 0) { throw "Installer smoke process exited with code $($process.ExitCode)." }
}

if ($TimeoutSeconds -lt 1 -or $TimeoutSeconds -gt 600) { throw 'TimeoutSeconds must be between 1 and 600.' }
if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) { throw 'RUNNER_TEMP must be set.' }
$runnerTemp = Get-FullPath $env:RUNNER_TEMP
if (-not (Test-Path -LiteralPath $runnerTemp -PathType Container)) { throw 'RUNNER_TEMP must be an existing directory.' }
$bundle = Get-FullPath $BundleDirectory
if (-not (Test-Path -LiteralPath $bundle -PathType Container)) { throw 'BundleDirectory must be an existing directory.' }
Assert-NoReparse $bundle
$installers = @(Get-ChildItem -LiteralPath $bundle -Filter '*.exe' -File -Recurse | Where-Object {
  (Get-FullPath $_.FullName).StartsWith("$bundle\", [StringComparison]::OrdinalIgnoreCase)
})
if ($installers.Count -ne 1) { throw 'Bundle must contain exactly one installer executable.' }
$installer = $installers[0]
if (($installer.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Installer must be a regular non-reparse file.' }

$run = if ($env:GITHUB_RUN_ID -match '^\d+$') { $env:GITHUB_RUN_ID } else { 'local' }
$attempt = if ($env:GITHUB_RUN_ATTEMPT -match '^\d+$') { $env:GITHUB_RUN_ATTEMPT } else { '0' }
$installRoot = Get-FullPath (Join-Path $runnerTemp "origami2-installer-smoke-$run-$attempt-$([guid]::NewGuid().ToString('N'))")
Assert-SafeDirectChild $installRoot $runnerTemp
if (Test-Path -LiteralPath $installRoot) { throw 'Installer smoke root already exists.' }

try {
  # NSIS requires /D to be the final argument.
  Invoke-Bounded $installer.FullName @('/S', "/D=$installRoot") $TimeoutSeconds
  if (-not (Test-Path -LiteralPath $installRoot -PathType Container)) { throw 'Installer did not create the expected root.' }
  Assert-NoReparse $installRoot
  foreach ($relative in @('ORIGAMI2.exe', 'uninstall.exe', 'fonts\NotoSansJP-Variable.ttf', 'licenses\NotoSansJP-OFL.txt')) {
    $path = Join-Path $installRoot $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Installed payload is missing: $relative" }
    if (((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Installed payload is a reparse point: $relative" }
  }
  Invoke-Bounded (Join-Path $installRoot 'uninstall.exe') @('/S') $TimeoutSeconds
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ((Test-Path -LiteralPath $installRoot) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
  if (Test-Path -LiteralPath $installRoot) { throw 'Silent uninstaller left installed payload behind.' }
} finally {
  if (Test-Path -LiteralPath $installRoot) {
    try { Assert-SafeDirectChild $installRoot $runnerTemp } catch { throw 'Refusing cleanup outside RUNNER_TEMP or through a reparse parent.' }
    Assert-NoReparse $installRoot
    Remove-Item -LiteralPath $installRoot -Recurse -Force
  }
}
