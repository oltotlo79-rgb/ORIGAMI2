$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$temp = Join-Path ([IO.Path]::GetTempPath()) "origami2 smoke fixture $([guid]::NewGuid().ToString('N'))"
$bundle = Join-Path $temp 'bundle'
New-Item -ItemType Directory -Path $bundle | Out-Null
$junctionTarget = Join-Path $temp 'junction-target'
New-Item -ItemType Directory -Path $junctionTarget | Out-Null
$source = @'
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
public static class Fixture {
  public static int Main(string[] args) {
    string self = Process.GetCurrentProcess().MainModule.FileName;
    string mode = Environment.GetEnvironmentVariable("ORIGAMI2_SMOKE_FIXTURE_MODE") ?? "success";
    if (Path.GetFileName(self).Equals("uninstall.exe", StringComparison.OrdinalIgnoreCase)) {
      string root = Path.GetDirectoryName(self);
      if (mode == "residue") return 0;
      Process.Start(new ProcessStartInfo("cmd.exe", "/d /c ping 127.0.0.1 -n 2 >nul & rmdir /s /q \"" + root + "\"") { CreateNoWindow = true, UseShellExecute = false });
      return 0;
    }
    if (mode == "nonzero") return 7;
    if (mode == "timeout") { Thread.Sleep(30000); return 0; }
    string install = args.LastOrDefault(x => x.StartsWith("/D=", StringComparison.Ordinal));
    if (install == null || args.Last() != install) return 9;
    string rootPath = install.Substring(3);
    Directory.CreateDirectory(rootPath);
    File.Copy(self, Path.Combine(rootPath, "ORIGAMI2.exe"));
    File.Copy(self, Path.Combine(rootPath, "uninstall.exe"));
    Directory.CreateDirectory(Path.Combine(rootPath, "fonts"));
    Directory.CreateDirectory(Path.Combine(rootPath, "licenses"));
    File.WriteAllText(Path.Combine(rootPath, "fonts", "NotoSansJP-Variable.ttf"), "fixture");
    File.WriteAllText(Path.Combine(rootPath, "licenses", "NotoSansJP-OFL.txt"), "fixture");
    if (mode == "reparse") {
      string target = Environment.GetEnvironmentVariable("ORIGAMI2_SMOKE_FIXTURE_JUNCTION_TARGET");
      Process.Start(new ProcessStartInfo("cmd.exe", "/d /c mklink /J \"" + Path.Combine(rootPath, "junction") + "\" \"" + target + "\"") { CreateNoWindow = true, UseShellExecute = false }).WaitForExit();
    }
    return 0;
  }
}
'@
try {
  $sourcePath = Join-Path $temp 'fixture.cs'
  [IO.File]::WriteAllText($sourcePath, $source, [Text.UTF8Encoding]::new($false))
  # GetFolderPath(Windows) queries the OS-known machine directory; SystemRoot is not a
  # registry-backed Machine environment value on standard Windows installations.
  $machineRoot = [Environment]::GetFolderPath([Environment+SpecialFolder]::Windows)
  if ([string]::IsNullOrWhiteSpace($machineRoot)) { throw 'The machine SystemRoot is unavailable.' }
  $machineRoot = [IO.Path]::GetFullPath($machineRoot).TrimEnd('\')
  foreach ($declaredRoot in @($env:SystemRoot, $env:WINDIR)) {
    if ([string]::IsNullOrWhiteSpace($declaredRoot) -or -not [string]::Equals(
      $machineRoot,
      [IO.Path]::GetFullPath($declaredRoot).TrimEnd('\'),
      [StringComparison]::OrdinalIgnoreCase
    )) { throw 'The process Windows root does not match the machine SystemRoot.' }
  }
  $csc = @(
    (Join-Path $machineRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'),
    (Join-Path $machineRoot 'Microsoft.NET\Framework\v4.0.30319\csc.exe')
  ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
  if (-not $csc) { throw 'A bounded local C# compiler is required for the installer smoke fixture.' }
  $cscItem = Get-Item -LiteralPath $csc -Force
  $component = $machineRoot
  if (((Get-Item -LiteralPath $component -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw 'The machine SystemRoot must not be a reparse point.'
  }
  if (-not $cscItem.FullName.StartsWith("$machineRoot\", [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The local C# compiler is outside the machine Windows directory.'
  }
  $relativeCompiler = $cscItem.FullName.Substring($machineRoot.Length).TrimStart('\')
  foreach ($part in $relativeCompiler.Split([IO.Path]::DirectorySeparatorChar)) {
    $component = Join-Path $component $part
    if (((Get-Item -LiteralPath $component -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw 'The local C# compiler path contains a reparse point.'
    }
  }
  if (-not ($cscItem -is [IO.FileInfo]) -or
      ($cscItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
      -not [string]::Equals($cscItem.FullName, [IO.Path]::GetFullPath($csc), [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The local C# compiler must be the exact regular system file.'
  }
  $fixture = Join-Path $bundle 'fixture.exe'
  & $cscItem.FullName /nologo /target:exe "/out:$fixture" $sourcePath
  if ($LASTEXITCODE -ne 0) { throw 'Could not compile the installer smoke fixture.' }
  $fixtureItem = Get-Item -LiteralPath $fixture -Force
  if (($fixtureItem -is [IO.FileInfo]) -and ($fixtureItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -and
      [string]::Equals($fixtureItem.FullName, [IO.Path]::GetFullPath($fixture), [StringComparison]::OrdinalIgnoreCase)) {
    # Exact regular output accepted.
  } else { throw 'The compiled installer smoke fixture is not an exact regular file.' }
  $env:RUNNER_TEMP = $temp
  $env:ORIGAMI2_SMOKE_FIXTURE_JUNCTION_TARGET = $junctionTarget
  $smoke = Join-Path $root '.github\scripts\smoke_windows_installer.ps1'
  function Invoke-Case([string]$Mode, [bool]$ShouldPass, [int]$Timeout = 3) {
    $env:ORIGAMI2_SMOKE_FIXTURE_MODE = $Mode
    $failed = $false
    try { & $smoke -BundleDirectory $bundle -TimeoutSeconds $Timeout } catch { $failed = $true }
    if ($failed -eq $ShouldPass) { throw "Unexpected smoke result for fixture mode '$Mode'." }
    Get-ChildItem -LiteralPath $temp -Directory -Filter 'origami2-installer-smoke-*' | ForEach-Object {
      $junction = Join-Path $_.FullName 'junction'
      if (Test-Path -LiteralPath $junction) { cmd.exe /d /c "rmdir `"$junction`"" | Out-Null }
      cmd.exe /d /c "rmdir /s /q `"$($_.FullName)`"" | Out-Null
    }
  }
  Invoke-Case success $true
  Invoke-Case nonzero $false
  Invoke-Case timeout $false 1
  Invoke-Case residue $false 1
  Invoke-Case reparse $false
} finally {
  $env:ORIGAMI2_SMOKE_FIXTURE_MODE = $null
  $env:ORIGAMI2_SMOKE_FIXTURE_JUNCTION_TARGET = $null
  if (Test-Path -LiteralPath $temp) { cmd.exe /d /c "rmdir /s /q `"$temp`"" | Out-Null }
}
