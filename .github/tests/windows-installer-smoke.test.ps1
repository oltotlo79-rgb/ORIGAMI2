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
  Add-Type -TypeDefinition $source -Language CSharp -OutputAssembly (Join-Path $bundle 'fixture.exe') -OutputType ConsoleApplication
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
